use super::beer_list::{PAGE_SIZE, list_url, page_number, query_encode};
use crate::components::photo::AuthenticatedPhoto;
use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use serde::Deserialize;
use wasm_bindgen::JsCast;

#[derive(Debug, Clone, Deserialize)]
struct TastingResponse {
    id: String,
    beer_id: String,
    beer_name: Option<String>,
    brewery_name: Option<String>,
    score: i32,
    serving_style: Option<String>,
    notes: Option<String>,
    location_id: Option<String>,
    location_name: Option<String>,
    session_name: Option<String>,
    tasted_at: String,
    #[serde(default)]
    has_photo: bool,
}

#[component]
pub fn MyTastingsPage(token: ReadSignal<Option<String>>) -> impl IntoView {
    let query = use_query_map();
    let page = Memo::new(move |_| page_number(query.with(|q| q.get("page"))));
    let beer = Memo::new(move |_| query.with(|q| q.get("beer")).unwrap_or_default());
    let (refresh, set_refresh) = signal(0u32);
    let tastings = LocalResource::new(move || {
        refresh.get();
        let token = token.get();
        let page = page.get();
        let beer = beer.get();
        async move {
            match token {
                Some(token) => fetch_tastings(&token, page, &beer).await,
                None => Ok(Vec::new()),
            }
        }
    });
    view! {
        <div class="page my-tastings">
            <div class="page-header"><h2>"My Tastings"</h2><a class="btn-primary" href=move || {
                if beer.get().is_empty() { "/tastings/new".into() } else { format!("/tastings/new?beer={}", query_encode(&beer.get())) }
            }>"Record a tasting"</a></div>
            <Show when=move || !beer.get().is_empty()><p>"Showing tastings of the selected beer. "<a href="/tastings">"Show all tastings"</a></p></Show>
            {move || if token.get().is_none() {
                view! { <p class="login-prompt"><a href=format!("/login?next={}", query_encode(&list_url("/tastings", "beer", &beer.get(), page.get())))>"Log in to see your tastings"</a></p> }.into_any()
            } else {
                view! {
                    <Suspense fallback=move || view! { <p role="status">"Loading tastings…"</p> }>
                        {move || tastings.get().map(|result| match &*result {
                            Ok(list) => {
                                let has_next = list.len() > PAGE_SIZE;
                                view! { <div class="tastings-timeline">
                                    {list.is_empty().then(|| view! { <p class="empty">{if page.get() > 1 { "No tastings on this page. Return to the previous page." } else { "No tastings yet for this selection. Record a tasting to start your history." }}</p> })}
                                    {list.iter().take(PAGE_SIZE).cloned().map(|tasting| view! { <TastingEntry tasting token refresh=set_refresh /> }).collect_view()}
                                    <nav class="pagination" aria-label="Tasting pages">
                                        {(page.get() > 1).then(|| view! { <a href=list_url("/tastings", "beer", &beer.get(), page.get() - 1)>"Previous"</a> })}
                                        <span>{format!("Page {}", page.get())}</span>
                                        {has_next.then(|| view! { <a href=list_url("/tastings", "beer", &beer.get(), page.get() + 1)>"Next"</a> })}
                                    </nav>
                                </div> }.into_any()
                            },
                            Err(error) => view! { <div><p class="error" role="alert">{error.clone()}</p><button on:click=move |_| set_refresh.update(|n| *n += 1)>"Retry"</button></div> }.into_any(),
                        })}
                    </Suspense>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn TastingEntry(
    tasting: TastingResponse,
    token: ReadSignal<Option<String>>,
    refresh: WriteSignal<u32>,
) -> impl IntoView {
    let actions = NodeRef::<leptos::html::Details>::new();
    let trigger = NodeRef::<leptos::html::Summary>::new();
    let score_input = NodeRef::<leptos::html::Input>::new();
    let keep_button = NodeRef::<leptos::html::Button>::new();
    let (editing, set_editing) = signal(false);
    let (confirming, set_confirming) = signal(false);
    let (pending, set_pending) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (score, set_score) = signal(tasting.score.to_string());
    let (notes, set_notes) = signal(tasting.notes.clone().unwrap_or_default());
    let original_score = tasting.score;
    let original_notes = tasting.notes.clone().unwrap_or_default();
    let save_row = tasting.clone();
    let delete_id = tasting.id.clone();
    let on_save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if pending.get() {
            return;
        }
        let Ok(value) = score.get().parse::<i32>() else {
            set_error.set(Some("Enter a whole score from 0 to 10.".into()));
            return;
        };
        if !(0..=10).contains(&value) {
            set_error.set(Some("Enter a whole score from 0 to 10.".into()));
            return;
        }
        let Some(auth) = token.get() else {
            set_error.set(Some("Please log in again before saving.".into()));
            return;
        };
        let row = save_row.clone();
        let note = notes.get();
        set_pending.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({"score": value, "notes": note, "serving_style": row.serving_style, "location_id": row.location_id});
            let result = mutate_tasting(&auth, &row.id, Some(body)).await;
            if token.try_get_untracked().flatten().as_deref() != Some(&auth) {
                return;
            }
            match result {
                Ok(()) => {
                    set_editing.set(false);
                    refresh.update(|n| *n += 1);
                }
                Err(message) => set_error.set(Some(message)),
            }
            set_pending.set(false);
        });
    };
    let on_delete = move |_| {
        if pending.get() {
            return;
        }
        let Some(auth) = token.get() else {
            set_error.set(Some("Please log in again before deleting.".into()));
            return;
        };
        let id = delete_id.clone();
        set_pending.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            let result = mutate_tasting(&auth, &id, None).await;
            if token.try_get_untracked().flatten().as_deref() != Some(&auth) {
                return;
            }
            match result {
                Ok(()) => {
                    set_confirming.set(false);
                    refresh.update(|n| *n += 1);
                }
                Err(message) => set_error.set(Some(message)),
            }
            set_pending.set(false);
        });
    };
    let score_id = format!("score-{}", tasting.id);
    let notes_id = format!("notes-{}", tasting.id);
    let action_label = format!(
        "Actions for {} on {}",
        tasting.beer_name.as_deref().unwrap_or("this tasting"),
        format_date(&tasting.tasted_at)
    );
    let close_actions = move || {
        if let Some(menu) = actions.get_untracked() {
            menu.set_open(false);
        }
    };
    let return_to_actions = move || {
        request_animation_frame(move || {
            if let Some(button) = trigger.get_untracked() {
                let _ = button.focus();
            }
        });
    };
    let photo_id = tasting.id.clone();
    let photo_alt = format!(
        "Tasting photo of {}",
        tasting.beer_name.as_deref().unwrap_or("this beer")
    );
    view! {
        <article class="tasting-entry" aria-busy=move || pending.get().to_string()>
            <div class="tasting-summary">
            <div class="tasting-header">
                <div class="tasting-heading-actions">
                    <span class="score-badge">{format!("{}/10", tasting.score)}</span>
                    <details node_ref=actions class="tasting-options" hidden=move || editing.get() || confirming.get()
                        on:keydown=move |ev| { if ev.key() == "Escape" { close_actions(); return_to_actions(); ev.stop_propagation(); } }
                        on:focusout=move |ev| {
                            let next = ev.related_target().and_then(|target| target.dyn_into::<web_sys::Node>().ok());
                            if actions.get_untracked().is_some_and(|menu| !menu.contains(next.as_ref())) { close_actions(); }
                        }
                    >
                        <summary node_ref=trigger aria-label=action_label title="Tasting actions"><span aria-hidden="true">"…"</span></summary>
                        <div class="tasting-options-list">
                            <button type="button" disabled=move || pending.get() on:click=move |_| {
                                close_actions(); set_error.set(None); set_editing.set(true);
                                request_animation_frame(move || { if let Some(input) = score_input.get_untracked() { let _ = input.focus(); } });
                            }>"Edit tasting"</button>
                            <button type="button" class="tasting-delete-option" disabled=move || pending.get() on:click=move |_| {
                                close_actions(); set_error.set(None); set_confirming.set(true);
                                request_animation_frame(move || { if let Some(button) = keep_button.get_untracked() { let _ = button.focus(); } });
                            }>"Delete tasting"</button>
                        </div>
                    </details>
                </div>
                <h3><a href=format!("/beers/{}", tasting.beer_id)>{tasting.beer_name.unwrap_or_else(|| "View beer".into())}</a></h3>
                <span class="brewery">{tasting.brewery_name.unwrap_or_default()}</span>
            </div>
            <div class="tasting-meta">
                <time datetime=tasting.tasted_at.clone()>{format_date(&tasting.tasted_at)}</time>
                {tasting.serving_style.map(|value| view! { <span class="serving">{value}</span> })}
                {tasting.location_name.map(|value| view! { <span class="location">{value}</span> })}
                {tasting.session_name.map(|value| view! { <span class="session">{value}</span> })}
            </div>
            </div>
            {if tasting.has_photo {
                view! { <TastingPhoto token tasting_id=photo_id alt=photo_alt /> }.into_any()
            } else {
                view! { <div class="tasting-photo-placeholder" aria-label="No tasting photo"><span aria-hidden="true">"▧"</span><small>"No photo"</small></div> }.into_any()
            }}
            {tasting.notes.map(|value| view! { <p class="tasting-notes">{value}</p> })}
            {move || error.get().map(|message| view! { <p role="alert" class="error">{message}</p> })}
            <form hidden=move || !editing.get() on:submit=on_save>
                <label for=score_id.clone()>"Score (0–10)"</label>
                <input node_ref=score_input id=score_id type="number" min="0" max="10" step="1" required prop:value=move || score.get() disabled=move || pending.get() on:input=move |ev| set_score.set(event_target_value(&ev)) />
                <label for=notes_id.clone()>"Private notes"</label>
                <textarea id=notes_id rows="3" prop:value=move || notes.get() disabled=move || pending.get() on:input=move |ev| set_notes.set(event_target_value(&ev))></textarea>
                <button type="submit" disabled=move || pending.get()>"Save changes"</button>
                <button type="button" disabled=move || pending.get() on:click=move |_| { set_score.set(original_score.to_string()); set_notes.set(original_notes.clone()); set_editing.set(false); set_error.set(None); return_to_actions(); }>"Cancel"</button>
            </form>
            <div hidden=move || !confirming.get()>
                <p>"Delete this tasting permanently?"</p>
                <button type="button" disabled=move || pending.get() on:click=on_delete>"Yes, delete tasting"</button>
                <button node_ref=keep_button type="button" disabled=move || pending.get() on:click=move |_| { set_confirming.set(false); set_error.set(None); return_to_actions(); }>"Keep tasting"</button>
            </div>
            <Show when=move || pending.get()><p role="status">"Saving changes…"</p></Show>
        </article>
    }
}

#[component]
fn TastingPhoto(
    token: ReadSignal<Option<String>>,
    tasting_id: String,
    alt: String,
) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    Effect::new(move |_| {
        token.get();
        set_expanded.set(false);
    });
    let thumbnail_id = tasting_id.clone();
    let thumbnail_alt = alt.clone();
    let section_id = format!("tasting-photo-{tasting_id}");
    view! {
        <div class="tasting-photo">
            <AuthenticatedPhoto token tasting_id=thumbnail_id alt=thumbnail_alt
                on_expand=Callback::new(move |_| set_expanded.set(true)) />
            <Show when=move || expanded.get()>
                <section id=section_id.clone() class="tasting-photo-expanded" aria-label="Enlarged tasting photo">
                    <button type="button" class="btn-secondary" on:click=move |_| set_expanded.set(false)>"Close enlarged photo"</button>
                    <AuthenticatedPhoto token tasting_id=tasting_id.clone() variant="large" alt=alt.clone() />
                </section>
            </Show>
        </div>
    }
}

fn format_date(iso: &str) -> String {
    iso.chars().take(10).collect()
}

async fn fetch_tastings(
    token: &str,
    page: usize,
    beer: &str,
) -> Result<Vec<TastingResponse>, String> {
    let mut url = format!(
        "/api/tastings?limit={}&offset={}",
        PAGE_SIZE + 1,
        (page - 1) * PAGE_SIZE
    );
    if !beer.is_empty() {
        url.push_str(&format!("&beer_id={}", query_encode(beer)));
    }
    let response = gloo_net::http::Request::get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|_| "Could not reach the server. Please try again.".to_string())?;
    if !response.ok() {
        return Err(if response.status() == 401 {
            "Your session expired. Please log in again."
        } else {
            "Could not load your tastings. Please try again."
        }
        .into());
    }
    response
        .json()
        .await
        .map_err(|_| "Could not read the tasting response.".into())
}

async fn mutate_tasting(
    token: &str,
    id: &str,
    body: Option<serde_json::Value>,
) -> Result<(), String> {
    let url = format!("/api/tastings/{id}");
    let response = match body {
        Some(body) => {
            gloo_net::http::Request::put(&url)
                .header("Authorization", &format!("Bearer {token}"))
                .json(&body)
                .map_err(|error| error.to_string())?
                .send()
                .await
        }
        None => {
            gloo_net::http::Request::delete(&url)
                .header("Authorization", &format!("Bearer {token}"))
                .send()
                .await
        }
    }
    .map_err(|_| {
        "Could not reach the server. Your changes were not confirmed. Please try again.".to_string()
    })?;
    if response.ok() {
        Ok(())
    } else {
        let status = response.status();
        let data = response
            .json::<serde_json::Value>()
            .await
            .unwrap_or_default();
        Err(data["error"]
            .as_str()
            .unwrap_or(if status == 401 {
                "Your session expired. Please log in again."
            } else {
                "Could not save this change. Please try again."
            })
            .to_string())
    }
}
