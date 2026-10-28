use super::add_beer::validate_beer_fields;
use crate::components::photo::{AuthenticatedPhoto, ObjectUrl};
use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_query_map};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wasm_bindgen_futures::JsFuture;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BeerSearchResult {
    id: String,
    name: String,
    style: Option<String>,
    abv: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct TastingCreated {
    id: String,
    score: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct LocationItem {
    id: String,
    name: String,
    location_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct SessionItem {
    id: String,
    name: String,
    join_code: String,
    ended_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BreweryOption {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CreatedBeerResponse {
    id: String,
    name: String,
}

const SERVING_STYLES: &[(&str, &str)] = &[
    ("draft", "🍺 Draft"),
    ("bottle", "🍾 Bottle"),
    ("can", "🥫 Can"),
    ("cask", "🪵 Cask"),
    ("crowler", "Crowler"),
    ("growler", "Growler"),
    ("nitro", "☁️ Nitro"),
    ("taster", "Taster"),
    ("other", "Other"),
];

#[component]
pub fn RateBeerPage(
    token: ReadSignal<Option<String>>,
    active_session: ReadSignal<Option<ActiveSession>>,
    set_active_session: WriteSignal<Option<ActiveSession>>,
) -> impl IntoView {
    let (search_query, set_search_query) = signal(String::new());
    let (search_results, set_search_results) = signal(Vec::<BeerSearchResult>::new());
    let (selected_beer, set_selected_beer) = signal(Option::<BeerSearchResult>::None);
    let (selected_score, set_selected_score) = signal(Option::<i32>::None);
    let (selected_serving, set_selected_serving) = signal(Option::<String>::None);
    let (notes, set_notes) = signal(String::new());
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<String>::None);
    let (searching, set_searching) = signal(false);
    // Raw photo bytes exist only in memory, never in a serialized draft.
    let (selected_photo, set_selected_photo) = signal(Option::<Arc<SelectedPhoto>>::None);
    let (reading_photo, set_reading_photo) = signal(false);
    let (photo_error, set_photo_error) = signal(Option::<String>::None);
    let (saved_tasting_id, set_saved_tasting_id) = signal(Option::<String>::None);
    let (photo_uploaded, set_photo_uploaded) = signal(false);
    let photo_generation = RwSignal::new(0_u64);

    // Location state — restore last used
    let (locations, set_locations) = signal(Vec::<LocationItem>::new());
    let (selected_location, set_selected_location) = signal(Option::<String>::None);

    // Session state
    let (sessions, set_sessions) = signal(Vec::<SessionItem>::new());
    let (show_create_session, set_show_create_session) = signal(false);
    let (new_session_name, set_new_session_name) = signal(String::new());
    let (creating_session, set_creating_session) = signal(false);

    // Inline beer creation state
    let (show_quick_add, set_show_quick_add) = signal(false);
    let (quick_beer_name, set_quick_beer_name) = signal(String::new());
    let (quick_brewery_id, set_quick_brewery_id) = signal(String::new());
    let (quick_new_brewery, set_quick_new_brewery) = signal(false);
    let (quick_brewery_name, set_quick_brewery_name) = signal(String::new());
    let (quick_style, set_quick_style) = signal(String::new());
    let (quick_abv, set_quick_abv) = signal(String::new());
    let (adding_beer, set_adding_beer) = signal(false);
    let (breweries, set_breweries) = signal(Vec::<BreweryOption>::new());

    let query = use_query_map();
    let location = use_location();
    let (draft_key, set_draft_key) = signal(String::new());
    Effect::new(move |_| {
        let key = tasting_draft_key(token.get().as_deref());
        let draft = load_draft(&key).unwrap_or_default();
        set_search_query.set(draft.search);
        set_selected_beer.set(draft.beer);
        set_selected_score.set(draft.score);
        set_selected_serving.set(draft.serving);
        set_selected_location.set(draft.location);
        set_notes.set(draft.notes);
        set_show_quick_add.set(draft.quick_add);
        set_quick_beer_name.set(draft.beer_name);
        set_quick_brewery_id.set(draft.brewery_id);
        set_quick_new_brewery.set(draft.new_brewery);
        set_quick_brewery_name.set(draft.brewery_name);
        set_quick_style.set(draft.style);
        set_quick_abv.set(draft.abv);
        set_search_results.set(Vec::new());
        set_success.set(None);
        set_error.set(None);
        set_submitting.set(false);
        photo_generation.update(|value| *value = value.wrapping_add(1));
        set_selected_photo.set(None);
        set_reading_photo.set(false);
        set_photo_error.set(None);
        set_saved_tasting_id.set(None);
        set_photo_uploaded.set(false);
        set_adding_beer.set(false);
        set_creating_session.set(false);
        set_show_create_session.set(false);
        set_new_session_name.set(String::new());
        set_draft_key.set(key);
    });
    Effect::new(move |_| {
        let key = draft_key.get();
        if key.is_empty() || key != tasting_draft_key(token.get().as_deref()) {
            return;
        }
        save_draft(
            &key,
            &TastingDraft {
                search: search_query.get(),
                beer: selected_beer.get(),
                score: selected_score.get(),
                serving: selected_serving.get(),
                location: selected_location.get(),
                notes: notes.get(),
                quick_add: show_quick_add.get(),
                beer_name: quick_beer_name.get(),
                brewery_id: quick_brewery_id.get(),
                new_brewery: quick_new_brewery.get(),
                brewery_name: quick_brewery_name.get(),
                style: quick_style.get(),
                abv: quick_abv.get(),
            },
        );
    });
    let (beer_request, set_beer_request) = signal(0_u64);
    let (loading_beer, set_loading_beer) = signal(false);
    let previous_beer_query = StoredValue::new(Option::<String>::None);
    Effect::new(move |_| {
        let beer_id = query.read().get("beer");
        let had_query = previous_beer_query.get_value().is_some();
        previous_beer_query.set_value(beer_id.clone());
        set_loading_beer.set(beer_id.is_some());
        if beer_id.is_none() && had_query {
            set_selected_beer.set(None);
        }
        let generation = beer_request.get_untracked() + 1;
        set_beer_request.set(generation);
        if let Some(id) = beer_id {
            set_selected_beer.set(None);
            set_error.set(None);
            leptos::task::spawn_local(async move {
                let result = fetch_beer(&id).await;
                if beer_request.try_get_untracked() != Some(generation) {
                    return;
                }
                set_loading_beer.set(false);
                match result {
                    Ok(beer) => set_selected_beer.set(Some(beer)),
                    Err(e) => set_error.set(Some(e)),
                }
            });
        }
    });
    Effect::new(move |_| {
        set_locations.set(Vec::new());
        set_sessions.set(Vec::new());
        if let Some(tok) = token.get() {
            let tok2 = tok.clone();
            leptos::task::spawn_local(async move {
                match fetch_locations(&tok).await {
                    Ok(locs) if token.get_untracked().as_deref() == Some(&tok) => {
                        set_locations.set(locs)
                    }
                    Err(e) if token.get_untracked().as_deref() == Some(&tok) => {
                        set_error.set(Some(e))
                    }
                    _ => {}
                }
            });
            leptos::task::spawn_local(async move {
                match fetch_active_sessions(&tok2).await {
                    Ok(sess) if token.get_untracked().as_deref() == Some(&tok2) => {
                        set_sessions.set(sess)
                    }
                    Err(e) if token.get_untracked().as_deref() == Some(&tok2) => {
                        set_error.set(Some(e))
                    }
                    _ => {}
                }
            });
        }
    });

    let on_search = move |_| {
        let query = search_query.get();
        if query.len() < 2 {
            return;
        }
        set_searching.set(true);
        set_show_quick_add.set(false);
        leptos::task::spawn_local(async move {
            match search_beers(&query).await {
                Ok(results) => {
                    set_search_results.set(results);
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_searching.set(false);
        });
    };

    let on_create_session = move |_| {
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        let name = new_session_name.get().trim().to_string();
        if creating_session.get() || name.is_empty() {
            return;
        }
        set_creating_session.set(true);
        leptos::task::spawn_local(async move {
            match create_session(&tok, &name).await {
                Ok(sess) => {
                    if token.get_untracked().as_deref() != Some(&tok) {
                        return;
                    }
                    set_active_session.set(Some(ActiveSession {
                        id: sess.id.clone(),
                        name: sess.name.clone(),
                    }));
                    set_sessions.update(|list| list.insert(0, sess));
                    set_new_session_name.set(String::new());
                    set_show_create_session.set(false);
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_creating_session.set(false);
        });
    };

    let on_select_session = move |id: String, name: String| {
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        let id2 = id.clone();
        let name2 = name.clone();
        leptos::task::spawn_local(async move {
            match join_session(&tok, &id2).await {
                Ok(()) if token.get_untracked().as_deref() == Some(&tok) => {
                    set_active_session.set(Some(ActiveSession {
                        id: id2,
                        name: name2,
                    }));
                }
                Err(e) => set_error.set(Some(e)),
                _ => {}
            }
        });
    };

    // Quick add beer handler
    let on_quick_add_beer = move |_| {
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        if adding_beer.get() {
            return;
        }
        let name = quick_beer_name.get().trim().to_string();
        let is_new_brewery = quick_new_brewery.get();
        let brewery_id = quick_brewery_id.get();
        let brewery_name = quick_brewery_name.get().trim().to_string();
        let style_val = quick_style.get();
        let abv_val = quick_abv.get();
        let abv_parsed =
            match validate_beer_fields(&name, &abv_val, is_new_brewery, &brewery_id, &brewery_name)
            {
                Ok(abv) => abv,
                Err(e) => {
                    set_error.set(Some(e));
                    return;
                }
            };
        set_adding_beer.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            // Resolve brewery
            let bid = if is_new_brewery {
                if brewery_name.is_empty() {
                    set_error.set(Some("Brewery name is required".into()));
                    set_adding_beer.set(false);
                    return;
                }
                match create_brewery_api(&tok, &brewery_name).await {
                    Ok(id) => {
                        if token.get_untracked().as_deref() != Some(&tok) {
                            return;
                        }
                        set_quick_brewery_id.set(id.clone());
                        set_quick_new_brewery.set(false);
                        set_breweries.update(|items| {
                            items.push(BreweryOption {
                                id: id.clone(),
                                name: brewery_name.clone(),
                            })
                        });
                        id
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                        set_adding_beer.set(false);
                        return;
                    }
                }
            } else {
                if brewery_id.is_empty() {
                    set_error.set(Some("Please select a brewery".into()));
                    set_adding_beer.set(false);
                    return;
                }
                brewery_id
            };

            match create_beer_api(
                &tok,
                &bid,
                &name,
                if style_val.is_empty() {
                    None
                } else {
                    Some(&style_val)
                },
                abv_parsed,
            )
            .await
            {
                Ok(created) => {
                    if token.get_untracked().as_deref() != Some(&tok) {
                        return;
                    }
                    // Auto-select the newly created beer
                    set_selected_beer.set(Some(BeerSearchResult {
                        id: created.id,
                        name: created.name,
                        style: if style_val.is_empty() {
                            None
                        } else {
                            Some(style_val)
                        },
                        abv: abv_parsed,
                    }));
                    set_show_quick_add.set(false);
                    set_quick_beer_name.set(String::new());
                    set_quick_brewery_id.set(String::new());
                    set_quick_new_brewery.set(false);
                    set_quick_brewery_name.set(String::new());
                    set_quick_style.set(String::new());
                    set_quick_abv.set(String::new());
                    set_search_results.set(Vec::new());
                    set_search_query.set(String::new());
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_adding_beer.set(false);
        });
    };

    // Show quick add and pre-fill with search query
    let on_show_quick_add = move |_| {
        set_show_quick_add.set(true);
        set_quick_beer_name.set(search_query.get());
        // Load breweries for the selector
        if let Some(tok) = token.get() {
            leptos::task::spawn_local(async move {
                if let Ok(list) = fetch_breweries_api().await {
                    set_breweries.set(list);
                }
                let _ = tok;
            });
        }
    };

    let on_photo_change = move |ev: leptos::ev::Event| {
        if submitting.get() {
            return;
        }
        let input = event_target::<web_sys::HtmlInputElement>(&ev);
        let file = input.files().and_then(|files| files.get(0));
        // Allow selecting the same file again after removing it or a validation error.
        input.set_value("");
        let Some(file) = file else {
            return;
        };
        photo_generation.update(|value| *value = value.wrapping_add(1));
        let generation = photo_generation.get_untracked();
        let owner = token.get_untracked();
        set_selected_photo.set(None);
        set_reading_photo.set(false);
        set_photo_error.set(None);
        let format = match validate_photo_file(&file.type_(), &file.name(), file.size() as u64) {
            Ok(format) => format,
            Err(message) => {
                set_photo_error.set(Some(message));
                return;
            }
        };
        set_reading_photo.set(true);
        leptos::task::spawn_local(async move {
            let bytes = JsFuture::from(file.array_buffer()).await;
            if photo_generation.try_get_untracked() != Some(generation)
                || token.try_get_untracked().flatten() != owner
            {
                return;
            }
            match bytes {
                Ok(buffer) => {
                    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                    // HEIC/HEIF browser decoding varies. Keep the original bounded
                    // bytes for the server and show its JPEG thumbnail after saving.
                    let preview = if format.browser_previewable() {
                        ObjectUrl::from_blob(&file).ok()
                    } else {
                        None
                    };
                    set_selected_photo.set(Some(Arc::new(SelectedPhoto {
                        bytes,
                        mime: format.mime().to_string(),
                        name: file.name(),
                        preview,
                    })));
                }
                Err(_) => set_photo_error.set(Some(
                    "Could not read this photo. Please select it again.".into(),
                )),
            }
            set_reading_photo.set(false);
        });
    };

    let on_retry_photo = move |_| {
        if submitting.get() {
            return;
        }
        let (Some(tok), Some(id), Some(photo)) =
            (token.get(), saved_tasting_id.get(), selected_photo.get())
        else {
            return;
        };
        let generation = photo_generation.get_untracked();
        set_submitting.set(true);
        set_photo_error.set(None);
        leptos::task::spawn_local(async move {
            let result = upload_tasting_photo(&tok, &id, &photo).await;
            if photo_generation.try_get_untracked() != Some(generation)
                || token.try_get_untracked().flatten().as_deref() != Some(&tok)
            {
                return;
            }
            match result {
                Ok(()) => {
                    set_selected_photo.set(None);
                    set_photo_uploaded.set(true);
                }
                Err(message) => set_photo_error.set(Some(message)),
            }
            set_submitting.set(false);
        });
    };

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        // A saved tasting may only retry its photo; never POST it again.
        if submitting.get() || reading_photo.get() || saved_tasting_id.get().is_some() {
            return;
        }

        let beer = match selected_beer.get() {
            Some(b) => b,
            None => {
                set_error.set(Some("Please select a beer".into()));
                return;
            }
        };
        let score = match selected_score.get() {
            Some(s) => s,
            None => {
                set_error.set(Some("Please select a score".into()));
                return;
            }
        };
        let tok = match token.get() {
            Some(t) => t,
            None => {
                set_error.set(Some("Please log in to rate".into()));
                return;
            }
        };

        set_submitting.set(true);
        set_error.set(None);
        set_success.set(None);
        set_photo_error.set(None);

        let notes_val = notes.get();
        let session_id = active_session.get().map(|s| s.id.clone());
        let location_id = selected_location.get();
        let serving = selected_serving.get();
        let photo = selected_photo.get();
        let generation = photo_generation.get_untracked();

        leptos::task::spawn_local(async move {
            match submit_tasting(
                &tok,
                &beer.id,
                score,
                serving.as_deref(),
                if notes_val.is_empty() {
                    None
                } else {
                    Some(&notes_val)
                },
                location_id.as_deref(),
                session_id.as_deref(),
            )
            .await
            {
                Ok(created) => {
                    if photo_generation.try_get_untracked() != Some(generation)
                        || token.try_get_untracked().flatten().as_deref() != Some(&tok)
                    {
                        return;
                    }
                    set_saved_tasting_id.set(Some(created.id.clone()));
                    set_success.set(Some(format!("Rated {} — {}/10! 🎉", beer.name, score)));
                    set_selected_beer.set(None);
                    set_selected_score.set(None);
                    // Keep serving style & location for next rating
                    set_notes.set(String::new());
                    set_search_query.set(String::new());
                    set_search_results.set(Vec::new());
                    if let Some(photo) = photo {
                        let result = upload_tasting_photo(&tok, &created.id, &photo).await;
                        if photo_generation.try_get_untracked() != Some(generation)
                            || token.try_get_untracked().flatten().as_deref() != Some(&tok)
                        {
                            return;
                        }
                        match result {
                            Ok(()) => {
                                set_selected_photo.set(None);
                                set_photo_uploaded.set(true);
                            }
                            Err(message) => set_photo_error.set(Some(message)),
                        }
                    }
                }
                Err(e) => {
                    if photo_generation.try_get_untracked() != Some(generation)
                        || token.try_get_untracked().flatten().as_deref() != Some(&tok)
                    {
                        return;
                    }
                    set_error.set(Some(e));
                }
            }
            set_submitting.set(false);
        });
    };

    view! {
        <div class="page rate-beer">
            <h2>"Record a tasting"</h2>

            {move || {
                if token.get().is_none() {
                    Some(view! { <p class="login-prompt"><a on:click=move |_| mark_guest_draft_for_login() href=move || format!("/login?next={}", js_sys::encode_uri_component(&format!("{}{}", location.pathname.get(), if location.search.get().is_empty() { String::new() } else { format!("?{}", location.search.get()) })))>"Log in to record this tasting"</a></p> })
                } else {
                    None
                }
            }}

            {move || success.get().map(|msg| view! {
                <div class="success" role="status">
                    <p>{msg}</p>
                    <a href="/tastings">"View tasting history and edit"</a>
                    {move || (submitting.get() && saved_tasting_id.get().is_some()).then(|| view! {
                        <p role="status">"Tasting saved. Uploading photo..."</p>
                    })}
                    {move || photo_uploaded.get().then(|| view! {
                        <p>"Photo saved."</p>
                        {move || saved_tasting_id.get().map(|id| view! {
                            <AuthenticatedPhoto token=token tasting_id=id alt="Saved tasting photo".to_string() />
                        })}
                    })}
                    {move || photo_error.get().map(|message| view! {
                        <div class="photo-error">
                            <p role="alert">"Your tasting is saved, but the photo was not uploaded. "{message}</p>
                            {move || selected_photo.get().map(|photo| view! {
                                {photo_selection_preview(&photo, "Photo waiting to upload")}
                                <p>{photo.name.clone()}</p>
                            })}
                            <button type="button" disabled=move || submitting.get() on:click=on_retry_photo>"Retry photo upload"</button>
                            <button type="button" disabled=move || submitting.get() on:click=move |_| {
                                set_selected_photo.set(None); set_photo_error.set(None);
                            }>"Continue without photo"</button>
                        </div>
                    })}
                    <button type="button" disabled=move || submitting.get() || photo_error.get().is_some() on:click=move |_| {
                        set_success.set(None); set_saved_tasting_id.set(None);
                        set_selected_photo.set(None); set_photo_uploaded.set(false);
                    }>"Record another tasting"</button>
                </div>
            })}
            {move || error.get().map(|e| view! { <div class="error" role="alert">{e}</div> })}

            {move || loading_beer.get().then(|| view! { <p role="status">"Loading selected beer..."</p> })}

            // Session selector
            <div class="session-picker" style:display=move || if token.get().is_some() && saved_tasting_id.get().is_none() { "block" } else { "none" }>
                {move || {
                    if let Some(s) = active_session.get() {
                        view! {
                            <div class="active-session-banner">
                                "📋 Session: " <strong>{s.name.clone()}</strong>
                                <button type="button" class="btn-text" on:click=move |_| set_active_session.set(None)>"Use no session"</button>
                            </div>
                        }.into_any()
                    } else {
                        let sess = sessions.get();
                        view! {
                            <div class="session-select-row">
                                <span>"Session (optional):"</span>
                                {if !sess.is_empty() {
                                    let items = sess.clone();
                                    view! {
                                        <div class="session-chips">
                                            {items.iter().map(|s| {
                                                let sid = s.id.clone();
                                                let sname = s.name.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="chip"
                                                        on:click=move |_| on_select_session(sid.clone(), sname.clone())
                                                    >{s.name.clone()}</button>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <span class="muted">"No active sessions"</span> }.into_any()
                                }}
                                <button type="button" class="btn-text" on:click=move |_| set_show_create_session.update(|v| *v = !*v)>
                                    {move || if show_create_session.get() { "Cancel" } else { "+ New" }}
                                </button>
                            </div>
                            {move || show_create_session.get().then(|| view! {
                                <div class="inline-create">
                                    <label for="new-session-name">"Session name"</label>
                                    <input
                                        type="text"
                                        placeholder="Session name..."
                                        id="new-session-name"
                                        prop:value=move || new_session_name.get()
                                        on:input=move |ev| set_new_session_name.set(event_target_value(&ev))
                                    />
                                    <button type="button" on:click=on_create_session disabled=move || creating_session.get()>
                                        {move || if creating_session.get() { "..." } else { "Create" }}
                                    </button>
                                </div>
                            })}
                        }.into_any()
                    }
                }}
            </div>

            <div class="beer-search" style:display=move || if token.get().is_some() && saved_tasting_id.get().is_none() { "block" } else { "none" }>
                <div class="search-bar">
                    <label for="tasting-search">"Search beers"</label>
                                    <input
                        type="text"
                        placeholder="Search for a beer..."
                        id="tasting-search"
                                        prop:value=move || search_query.get()
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                let query = search_query.get();
                                if query.len() >= 2 {
                                    set_searching.set(true);
                                    set_show_quick_add.set(false);
                                    leptos::task::spawn_local(async move {
                                        match search_beers(&query).await {
                                            Ok(results) => set_search_results.set(results),
                                            Err(e) => set_error.set(Some(e)),
                                        }
                                        set_searching.set(false);
                                    });
                                }
                            }
                        }
                    />
                    <button type="button" on:click=on_search disabled=move || searching.get()>
                        {move || if searching.get() { "..." } else { "Search" }}
                    </button>
                </div>

                {move || {
                    let results = search_results.get();
                    let sel = selected_beer.get();
                    if !results.is_empty() && sel.is_none() {
                        Some(view! {
                            <div class="search-results">
                                {results.iter().map(|beer| {
                                    let b = beer.clone();
                                    view! {
                                        <button type="button"
                                            class="search-result-item"
                                            on:click=move |_| {
                                                set_selected_beer.set(Some(b.clone()));
                                                set_search_results.set(Vec::new());
                                            }
                                        >
                                            <strong>{beer.name.clone()}</strong>
                                            {beer.style.as_ref().map(|s| format!(" • {s}"))}
                                            {beer.abv.map(|a| format!(" • {:.1}%", a))}
                                        </button>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        })
                    } else {
                        None
                    }
                }}

                // "Not found? Quick-add" button
                {move || {
                    let results = search_results.get();
                    let sel = selected_beer.get();
                    let query = search_query.get();
                    if sel.is_none() && query.len() >= 2 && !searching.get() && !show_quick_add.get() {
                        Some(view! {
                            <button
                                type="button"
                                class="btn-text quick-add-trigger"
                                on:click=on_show_quick_add
                            >
                                {if results.is_empty() {
                                    "Beer not in database? Add it quick →"
                                } else {
                                    "Not the right one? Add a new beer →"
                                }}
                            </button>
                        })
                    } else {
                        None
                    }
                }}

                // Inline quick-add beer form
                {move || show_quick_add.get().then(|| view! {
                    <div class="quick-add-form">
                        <h4>"Quick Add Beer"</h4>
                        <div class="form-group">
                            <label for="quick-beer-name">"Beer name"</label>
                                    <input
                                type="text"
                                placeholder="Beer name"
                                id="quick-beer-name"
                                        prop:value=move || quick_beer_name.get()
                                on:input=move |ev| set_quick_beer_name.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="form-group">
                            {move || {
                                if quick_new_brewery.get() {
                                    view! {
                                        <div class="inline-create">
                                            <label for="quick-brewery-name">"Brewery name"</label>
                                    <input
                                                type="text"
                                                placeholder="New brewery name"
                                                id="quick-brewery-name"
                                        prop:value=move || quick_brewery_name.get()
                                                on:input=move |ev| set_quick_brewery_name.set(event_target_value(&ev))
                                            />
                                            <button type="button" class="btn-text" on:click=move |_| set_quick_new_brewery.set(false)>"Cancel"</button>
                                        </div>
                                    }.into_any()
                                } else {
                                    let brew_list = breweries.get();
                                    view! {
                                        <div class="inline-create">
                                            <label for="quick-beer-brewery">"Brewery"</label>
                                            <select id="quick-beer-brewery"
                                                prop:value=move || quick_brewery_id.get()
                                                on:change=move |ev| set_quick_brewery_id.set(event_target_value(&ev))
                                            >
                                                <option value="">"— Select brewery —"</option>
                                                {brew_list.iter().map(|b| {
                                                    let id = b.id.clone();
                                                    let name = b.name.clone();
                                                    view! { <option value=id>{name}</option> }
                                                }).collect::<Vec<_>>()}
                                            </select>
                                            <button type="button" class="btn-text" on:click=move |_| set_quick_new_brewery.set(true)>"+ New"</button>
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="quick-beer-style">"Style (optional)"</label>
                                    <input
                                    type="text"
                                    placeholder="Style (optional)"
                                    id="quick-beer-style"
                                        prop:value=move || quick_style.get()
                                    on:input=move |ev| set_quick_style.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="form-group">
                                <label for="quick-beer-abv">"ABV % (optional)"</label>
                                    <input
                                    type="number"
                                    step="0.1" min="0" max="100"
                                    placeholder="ABV %"
                                    id="quick-beer-abv"
                                        prop:value=move || quick_abv.get()
                                    on:input=move |ev| set_quick_abv.set(event_target_value(&ev))
                                />
                            </div>
                        </div>
                        <div class="quick-add-actions">
                            <button type="button" class="btn-primary" on:click=on_quick_add_beer disabled=move || adding_beer.get()>
                                {move || if adding_beer.get() { "Adding..." } else { "Add & Select" }}
                            </button>
                            <button type="button" class="btn-text" on:click=move |_| set_show_quick_add.set(false)>"Cancel"</button>
                        </div>
                    </div>
                })}

                {move || selected_beer.get().map(|beer| view! {
                    <div class="selected-beer">
                        <span>"Selected: " <strong>{beer.name.clone()}</strong></span>
                        <button type="button" class="btn-text" on:click=move |_| set_selected_beer.set(None)>"✕ Change"</button>
                    </div>
                })}
            </div>

            <form
                on:submit=on_submit
                style:display=move || if token.get().is_some() && selected_beer.get().is_some() && saved_tasting_id.get().is_none() { "block" } else { "none" }
            >
                <fieldset class="tasting-form-fields" disabled=move || submitting.get()>
                <legend>"Tasting details"</legend>
                <span id="score-label" class="form-label">"Your score"</span>
                <div class="score-selector" role="group" aria-labelledby="score-label">
                    {(0..=10i32).map(|n| {
                        view! {
                            <button
                                type="button"
                                class=move || {
                                    let base = match n {
                                        0..=2 => "score-btn poor",
                                        3..=4 => "score-btn below-avg",
                                        5 => "score-btn average",
                                        6..=7 => "score-btn good",
                                        8..=9 => "score-btn excellent",
                                        10 => "score-btn world-class",
                                        _ => "score-btn",
                                    };
                                    if selected_score.get() == Some(n) {
                                        format!("{base} selected")
                                    } else {
                                        base.to_string()
                                    }
                                }
                                aria-pressed=move || selected_score.get() == Some(n)
                                aria-label=format!("{n} out of 10")
                                on:click=move |_| set_selected_score.set(Some(n))
                            >
                                {n}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                <span id="serving-label" class="form-label">"Serving style"</span>
                <div class="serving-selector" role="group" aria-labelledby="serving-label">
                    {SERVING_STYLES.iter().map(|(val, label)| {
                        let v = val.to_string();
                        let v2 = val.to_string();
                        let pressed_value = val.to_string();
                        let l = *label;
                        view! {
                            <button
                                type="button"
                                class=move || {
                                    if selected_serving.get().as_deref() == Some(&v) {
                                        "chip selected"
                                    } else {
                                        "chip"
                                    }
                                }
                                aria-pressed=move || selected_serving.get().as_deref() == Some(&pressed_value)
                                on:click=move |_| {
                                    if selected_serving.get().as_deref() == Some(&v2) {
                                        set_selected_serving.set(None);
                                    } else {
                                        set_selected_serving.set(Some(v2.clone()));
                                    }
                                }
                            >
                                {l}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                // Location picker
                <div class="form-group">
                    <label for="tasting-location">"Location (optional)"</label>
                    <select id="tasting-location"
                        prop:value=move || selected_location.get().unwrap_or_default()
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            set_selected_location.set(if val.is_empty() { None } else { Some(val) });
                        }
                    >
                        <option value="">"— No location —"</option>
                        {move || locations.get().iter().map(|loc| {
                            view! {
                                <option value={loc.id.clone()}>
                                    {format!("{} ({})", loc.name, loc.location_type)}
                                </option>
                            }
                        }).collect::<Vec<_>>()}
                    </select>
                </div>

                <div class="form-group">
                    <label for="tasting-notes">"Notes (optional, encrypted)"</label>
                    <textarea id="tasting-notes"
                        prop:value=move || notes.get()
                        on:input=move |ev| set_notes.set(event_target_value(&ev))
                        rows="2"
                        placeholder="Your private tasting notes..."
                    ></textarea>
                </div>

                <fieldset class="tasting-photo-picker" disabled=move || submitting.get() || reading_photo.get()>
                    <legend>"Photo (optional)"</legend>
                    <p id="photo-help" class="hint">"Take a photo or choose one JPEG, PNG, WebP, HEIC, or HEIF, up to 12 MiB. HEIC/HEIF previews appear after saving."</p>
                    <div class="photo-inputs">
                        <div class="form-group">
                            <label for="tasting-camera">"Take a photo"</label>
                            <input id="tasting-camera" type="file" accept="image/*"
                                capture="environment" aria-describedby="photo-help" on:change=on_photo_change />
                        </div>
                        <div class="form-group">
                            <label for="tasting-photo-file">"Choose from library or files"</label>
                            <input id="tasting-photo-file" type="file" accept="image/jpeg,image/png,image/webp,image/heic,image/heif,.jpg,.jpeg,.png,.webp,.heic,.heif"
                                aria-describedby="photo-help" on:change=on_photo_change />
                        </div>
                    </div>
                    <p class="hint">"On supported phones, Take a photo opens the camera. If your browser opens a file picker instead, use its camera option or take a photo first and choose it from your library."</p>
                    {move || reading_photo.get().then(|| view! { <p role="status">"Reading photo..."</p> })}
                    {move || selected_photo.get().map(|photo| view! {
                        <div class="selected-photo">
                            {photo_selection_preview(&photo, "Selected tasting photo")}
                            <p>{photo.name.clone()}</p>
                            <button type="button" on:click=move |_| {
                                photo_generation.update(|value| *value = value.wrapping_add(1));
                                set_selected_photo.set(None); set_photo_error.set(None);
                            }>"Remove photo"</button>
                        </div>
                    })}
                    {move || photo_error.get().map(|message| view! { <p class="error" role="alert">{message}</p> })}
                </fieldset>
                <button type="submit" class="btn-primary" disabled=move || submitting.get() || reading_photo.get()>
                    {move || if submitting.get() { "Submitting..." } else { "Submit Tasting" }}
                </button>
                </fieldset>
            </form>
        </div>
    }
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub id: String,
    pub name: String,
}

/// Clear the active session from localStorage.
pub fn clear_persisted_session() {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item("open_tappd_session_id");
        let _ = storage.remove_item("open_tappd_session_name");
    }
}

async fn search_beers(query: &str) -> Result<Vec<BeerSearchResult>, String> {
    let resp = gloo_net::http::Request::get(&format!(
        "/api/beers?search={}",
        js_sys::encode_uri_component(query)
    ))
    .send()
    .await
    .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<BeerSearchResult>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to search beers".to_string())
    }
}

async fn fetch_locations(token: &str) -> Result<Vec<LocationItem>, String> {
    let resp = gloo_net::http::Request::get("/api/locations?limit=100")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<LocationItem>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Could not load locations. Your draft is preserved; try again.".into())
    }
}

async fn fetch_active_sessions(token: &str) -> Result<Vec<SessionItem>, String> {
    let resp = gloo_net::http::Request::get("/api/tasting-sessions?active_only=true&limit=50")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<SessionItem>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Could not load tasting sessions. Your draft is preserved; try again.".into())
    }
}

async fn create_session(token: &str, name: &str) -> Result<SessionItem, String> {
    let body = serde_json::json!({
        "name": name,
        "visibility": "public",
    });

    let resp = gloo_net::http::Request::post("/api/tasting-sessions")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<SessionItem>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to create session")
            .to_string())
    }
}

async fn join_session(token: &str, session_id: &str) -> Result<(), String> {
    let resp = gloo_net::http::Request::post(&format!("/api/tasting-sessions/{session_id}/join"))
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() || resp.status() == 200 {
        Ok(())
    } else {
        Err("Failed to join session".to_string())
    }
}

async fn submit_tasting(
    token: &str,
    beer_id: &str,
    score: i32,
    serving_style: Option<&str>,
    notes: Option<&str>,
    location_id: Option<&str>,
    session_id: Option<&str>,
) -> Result<TastingCreated, String> {
    let mut body = serde_json::json!({
        "beer_id": beer_id,
        "score": score,
    });
    if let Some(s) = serving_style {
        body["serving_style"] = serde_json::Value::String(s.to_string());
    }
    if let Some(n) = notes {
        body["notes"] = serde_json::Value::String(n.to_string());
    }
    if let Some(lid) = location_id {
        body["location_id"] = serde_json::Value::String(lid.to_string());
    }
    if let Some(sid) = session_id {
        body["session_id"] = serde_json::Value::String(sid.to_string());
    }

    let resp = gloo_net::http::Request::post("/api/tastings")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<TastingCreated>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        let data: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::json!({"error": "Failed to submit"}));
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to submit tasting")
            .to_string())
    }
}

// --- API helpers for inline beer creation ---

async fn fetch_breweries_api() -> Result<Vec<BreweryOption>, String> {
    let resp = gloo_net::http::Request::get("/api/breweries")
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<BreweryOption>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to fetch breweries".to_string())
    }
}

async fn create_brewery_api(token: &str, name: &str) -> Result<String, String> {
    let body = serde_json::json!({ "name": name });

    let resp = gloo_net::http::Request::post("/api/breweries")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
        data["id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No ID in response".to_string())
    } else {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to create brewery")
            .to_string())
    }
}

async fn create_beer_api(
    token: &str,
    brewery_id: &str,
    name: &str,
    style: Option<&str>,
    abv: Option<f64>,
) -> Result<CreatedBeerResponse, String> {
    let mut body = serde_json::json!({
        "brewery_id": brewery_id,
        "name": name,
    });
    if let Some(s) = style {
        body["style"] = serde_json::Value::String(s.to_string());
    }
    if let Some(a) = abv {
        body["abv"] = serde_json::json!(a);
    }

    let resp = gloo_net::http::Request::post("/api/beers")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<CreatedBeerResponse>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to create beer")
            .to_string())
    }
}

// Drafts contain private notes. Keep them within this browser tab and clear on logout.
const DRAFT_PREFIX: &str = "open_tappd_tasting_draft:";
const GUEST_TRANSFER: &str = "open_tappd_tasting_guest_login";

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct TastingDraft {
    search: String,
    beer: Option<BeerSearchResult>,
    score: Option<i32>,
    serving: Option<String>,
    location: Option<String>,
    notes: String,
    quick_add: bool,
    beer_name: String,
    brewery_id: String,
    new_brewery: bool,
    brewery_name: String,
    style: String,
    abv: String,
}

fn draft_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

fn tasting_draft_key(token: Option<&str>) -> String {
    // Read the public subject claim solely as a storage namespace. Never store a token.
    let subject = token
        .and_then(|token| token.split('.').nth(1))
        .and_then(decode_base64url)
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|claims| claims.get("sub")?.as_str().map(str::to_string));
    format!("{DRAFT_PREFIX}{}", subject.as_deref().unwrap_or("guest"))
}

fn decode_base64url(input: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let (mut buffer, mut bits) = (0_u32, 0_u32);
    for ch in input.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}

fn mark_guest_draft_for_login() {
    if let Some(storage) = draft_storage() {
        let _ = storage.set_item(GUEST_TRANSFER, "yes");
    }
}

fn load_draft(key: &str) -> Option<TastingDraft> {
    let storage = draft_storage()?;
    let raw = storage.get_item(key).ok().flatten();
    if key != format!("{DRAFT_PREFIX}guest")
        && storage.get_item(GUEST_TRANSFER).ok().flatten().is_some()
    {
        let _ = storage.remove_item(GUEST_TRANSFER);
        let guest_key = format!("{DRAFT_PREFIX}guest");
        let guest = storage.get_item(&guest_key).ok().flatten();
        let _ = storage.remove_item(&guest_key);
        // A signed-out page saves an empty guest draft too. Never let that
        // replace this account's existing work after reauthentication.
        return raw
            .or(guest)
            .and_then(|raw| serde_json::from_str(&raw).ok());
    }
    raw.and_then(|raw| serde_json::from_str(&raw).ok())
}

fn save_draft(key: &str, draft: &TastingDraft) {
    if let (Some(storage), Ok(raw)) = (draft_storage(), serde_json::to_string(draft)) {
        let _ = storage.set_item(key, &raw);
    }
}

pub fn clear_tasting_drafts() {
    if let Some(storage) = draft_storage() {
        let keys: Vec<_> = (0..storage.length().unwrap_or(0))
            .filter_map(|index| storage.key(index).ok().flatten())
            .filter(|key| key.starts_with(DRAFT_PREFIX) || key == GUEST_TRANSFER)
            .collect();
        for key in keys {
            let _ = storage.remove_item(&key);
        }
    }
}

async fn fetch_beer(id: &str) -> Result<BeerSearchResult, String> {
    let response =
        gloo_net::http::Request::get(&format!("/api/beers/{}", js_sys::encode_uri_component(id)))
            .send()
            .await
            .map_err(|e| format!("Could not load selected beer: {e}"))?;
    if response.ok() {
        response
            .json()
            .await
            .map_err(|e| format!("Could not read selected beer: {e}"))
    } else {
        Err("The selected beer could not be loaded. Search for another beer or try again.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_base64url, tasting_draft_key, validate_photo_file, PhotoFormat, MAX_PHOTO_BYTES,
    };

    #[test]
    fn photo_selection_accepts_supported_formats_and_enforces_size_before_reading() {
        for mime in [
            "image/jpeg",
            "image/png",
            "image/webp",
            "image/heic",
            "image/heif",
        ] {
            assert!(validate_photo_file(mime, "photo", MAX_PHOTO_BYTES).is_ok());
            assert!(validate_photo_file(mime, "photo", 1).is_ok());
            assert!(validate_photo_file(mime, "photo", 0).is_err());
            assert!(validate_photo_file(mime, "photo", MAX_PHOTO_BYTES + 1).is_err());
        }
        for mime in [
            "image/heic-sequence",
            "image/heif-sequence",
            "image/gif",
            "image/svg+xml",
            "text/plain",
        ] {
            assert!(
                validate_photo_file(mime, "photo.heic", 1024).is_err(),
                "{mime}"
            );
        }
    }

    #[test]
    fn missing_mime_uses_case_insensitive_filename_extension_without_guessing_arbitrary_types() {
        for (filename, expected) in [
            ("camera.JPG", PhotoFormat::Jpeg),
            ("camera.jpeg", PhotoFormat::Jpeg),
            ("camera.PNG", PhotoFormat::Png),
            ("camera.webp", PhotoFormat::WebP),
            ("camera.HEIC", PhotoFormat::Heic),
            ("camera.HeIf", PhotoFormat::Heif),
        ] {
            assert_eq!(validate_photo_file("", filename, 100), Ok(expected));
        }
        for filename in [
            "photo",
            "photo.",
            "photo.heic.exe",
            "photo.gif",
            "photo.svg",
        ] {
            assert!(
                validate_photo_file("", filename, 100).is_err(),
                "{filename}"
            );
        }
        assert_eq!(
            validate_photo_file("IMAGE/HEIC", "camera", 100),
            Ok(PhotoFormat::Heic)
        );
        assert_eq!(
            validate_photo_file("image/jpeg", "camera.heic", 100),
            Ok(PhotoFormat::Jpeg)
        );
    }

    #[test]
    fn heic_is_uploaded_in_original_format_without_a_browser_preview() {
        for format in [PhotoFormat::Heic, PhotoFormat::Heif] {
            assert!(!format.browser_previewable());
            assert!(format.mime().starts_with("image/hei"));
        }
        for format in [PhotoFormat::Jpeg, PhotoFormat::Png, PhotoFormat::WebP] {
            assert!(format.browser_previewable());
        }
    }

    #[test]
    fn drafts_are_scoped_by_subject_without_persisting_the_token() {
        let account_a = tasting_draft_key(Some("header.eyJzdWIiOiJ1c2VyLWEifQ.signature"));
        let account_b = tasting_draft_key(Some("header.eyJzdWIiOiJ1c2VyLWIifQ.signature"));
        assert_eq!(account_a, "open_tappd_tasting_draft:user-a");
        assert_ne!(account_a, account_b);
        assert_eq!(tasting_draft_key(None), "open_tappd_tasting_draft:guest");
        assert!(decode_base64url("!").is_none());
    }
}

const MAX_PHOTO_BYTES: u64 = 12 * 1024 * 1024;

struct SelectedPhoto {
    bytes: Vec<u8>,
    mime: String,
    name: String,
    preview: Option<ObjectUrl>,
}

fn photo_selection_preview(photo: &SelectedPhoto, alt: &'static str) -> AnyView {
    match &photo.preview {
        Some(preview) => view! {
            <img class="tasting-photo-preview" src=preview.as_str().to_string() alt=alt />
        }.into_any(),
        None => view! {
            <p class="photo-preview-unavailable" role="status">"Photo ready to upload. Preview will appear after saving."</p>
        }.into_any(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhotoFormat {
    Jpeg,
    Png,
    WebP,
    Heic,
    Heif,
}

impl PhotoFormat {
    fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::WebP => "image/webp",
            Self::Heic => "image/heic",
            Self::Heif => "image/heif",
        }
    }

    fn browser_previewable(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png | Self::WebP)
    }
}

fn validate_photo_file(mime: &str, filename: &str, size: u64) -> Result<PhotoFormat, String> {
    if size == 0 {
        return Err("This photo is empty. Please choose another file.".into());
    }
    if size > MAX_PHOTO_BYTES {
        return Err("This photo is too large. The limit is 12 MiB.".into());
    }
    // Some native file providers omit MIME types, especially for HEIC/HEIF.
    // Only use the filename fallback for an omitted type; the server verifies bytes.
    let format = match mime.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" => Some(PhotoFormat::Jpeg),
        "image/png" => Some(PhotoFormat::Png),
        "image/webp" => Some(PhotoFormat::WebP),
        "image/heic" => Some(PhotoFormat::Heic),
        "image/heif" => Some(PhotoFormat::Heif),
        "" => match filename
            .rsplit_once('.')
            .map(|(_, suffix)| suffix.to_ascii_lowercase())
            .as_deref()
        {
            Some("jpg" | "jpeg") => Some(PhotoFormat::Jpeg),
            Some("png") => Some(PhotoFormat::Png),
            Some("webp") => Some(PhotoFormat::WebP),
            Some("heic") => Some(PhotoFormat::Heic),
            Some("heif") => Some(PhotoFormat::Heif),
            _ => None,
        },
        _ => None,
    };
    format.ok_or_else(|| "Choose a JPEG, PNG, WebP, HEIC, or HEIF photo.".to_string())
}

async fn upload_tasting_photo(
    token: &str,
    tasting_id: &str,
    photo: &SelectedPhoto,
) -> Result<(), String> {
    let response = gloo_net::http::Request::put(&format!("/api/tastings/{tasting_id}/photo"))
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", &photo.mime)
        .body(js_sys::Uint8Array::from(photo.bytes.as_slice()))
        .map_err(|_| "Could not prepare the photo upload. Try again.".to_string())?
        .send()
        .await
        .map_err(|_| "Could not reach the server. Try again.".to_string())?;
    if response.ok() {
        return Ok(());
    }
    let status = response.status();
    let data: serde_json::Value = response.json().await.unwrap_or_default();
    Err(data["error"]
        .as_str()
        .unwrap_or(if status == 413 {
            "This photo is too large. The limit is 12 MiB."
        } else {
            "Could not upload the photo. Try again."
        })
        .to_string())
}
