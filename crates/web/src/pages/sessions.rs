use super::beer_list::{PAGE_SIZE, list_url, page_number, query_encode};
use crate::pages::rate_beer::ActiveSession;
use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use serde::{Deserialize, de::DeserializeOwned};

#[derive(Debug, Clone, Deserialize)]
struct SessionResponse {
    id: String,
    name: String,
    description: Option<String>,
    join_code: String,
    created_by: String,
    ended_at: Option<String>,
    participants: Option<Vec<ParticipantInfo>>,
}
#[derive(Debug, Clone, Deserialize)]
struct ParticipantInfo {
    username: String,
}
#[derive(Debug, Clone, Deserialize)]
struct SessionTasting {
    beer_id: String,
    beer_name: Option<String>,
    score: i32,
}

#[component]
pub fn SessionBrowserPage(
    token: ReadSignal<Option<String>>,
    active_session: ReadSignal<Option<ActiveSession>>,
    set_active_session: WriteSignal<Option<ActiveSession>>,
) -> impl IntoView {
    let query = use_query_map();
    let page = Memo::new(move |_| page_number(query.with(|q| q.get("page"))));
    let (code, set_code) = signal(query.with_untracked(|q| q.get("code")).unwrap_or_default());
    Effect::new(move |_| {
        if let Some(code) = query.with(|q| q.get("code")) {
            set_code.set(code);
        }
    });
    let (name, set_name) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (pending, set_pending) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (success, set_success) = signal(None::<String>);
    let (invitation, set_invitation) = signal(None::<String>);
    let (refresh, set_refresh) = signal(0u32);
    Effect::new(move |_| {
        token.get();
        set_pending.set(false);
        set_error.set(None);
        set_success.set(None);
        set_invitation.set(None);
    });
    let sessions = LocalResource::new(move || {
        let token = token.get();
        let page = page.get();
        refresh.get();
        async move {
            match token {
                None => Ok(Vec::<SessionResponse>::new()),
                Some(token) => {
                    get_json(
                        &token,
                        &format!(
                            "/api/tasting-sessions?limit={}&offset={}",
                            PAGE_SIZE + 1,
                            (page - 1) * PAGE_SIZE
                        ),
                    )
                    .await
                }
            }
        }
    });
    let on_join = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if pending.get() {
            return;
        }
        let Some(auth) = token.get() else {
            return;
        };
        let code = code.get().trim().to_uppercase();
        if code.is_empty() {
            set_error.set(Some("Enter an invitation code.".into()));
            return;
        }
        set_pending.set(true);
        set_error.set(None);
        set_success.set(None);
        leptos::task::spawn_local(async move {
            let result = post_json::<SessionResponse>(
                &auth,
                "/api/tasting-sessions/join",
                serde_json::json!({"code": code}),
            )
            .await;
            if token.try_get_untracked().flatten().as_deref() != Some(&auth) {
                return;
            }
            match result {
                Ok(session) => {
                    set_active_session.set(Some(ActiveSession {
                        id: session.id,
                        name: session.name.clone(),
                    }));
                    set_success.set(Some(format!(
                        "Joined {}. New tastings will use this session.",
                        session.name
                    )));
                    set_invitation.set(Some(session.join_code));
                    set_refresh.update(|n| *n += 1);
                }
                Err(message) => set_error.set(Some(message)),
            }
            set_pending.set(false);
        });
    };
    let on_create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if pending.get() {
            return;
        }
        let Some(auth) = token.get() else {
            return;
        };
        let name = name.get().trim().to_string();
        if name.is_empty() || name.chars().count() > 200 {
            set_error.set(Some("Enter a session name of 1 to 200 characters.".into()));
            return;
        }
        let description = description.get();
        set_pending.set(true);
        set_error.set(None);
        set_success.set(None);
        leptos::task::spawn_local(async move {
            let result = post_json::<SessionResponse>(&auth, "/api/tasting-sessions", serde_json::json!({"name": name, "description": description, "visibility": "participants"})).await;
            if token.try_get_untracked().flatten().as_deref() != Some(&auth) {
                return;
            }
            match result {
                Ok(session) => {
                    set_active_session.set(Some(ActiveSession {
                        id: session.id,
                        name: session.name.clone(),
                    }));
                    set_success.set(Some(format!(
                        "Created {}. Share its invitation to invite participants.",
                        session.name
                    )));
                    set_invitation.set(Some(session.join_code));
                    set_name.set(String::new());
                    set_description.set(String::new());
                    set_refresh.update(|n| *n += 1);
                }
                Err(message) => set_error.set(Some(message)),
            }
            set_pending.set(false);
        });
    };
    view! {
        <div class="page sessions">
            <h2>"Tasting Sessions"</h2>
            {move || if token.get().is_none() {
                view! { <p class="login-prompt"><a href=format!("/login?next={}", query_encode(&list_url("/sessions", "code", &code.get(), page.get())))>"Log in to create or join a session"</a></p> }.into_any()
            } else {
                view! {
                    {move || active_session.get().map(|session| view! {
                        <div class="active-session-banner">"Selected for new tastings: "<strong>{session.name}</strong>
                            <button type="button" on:click=move |_| set_active_session.set(None)>"Use no session"</button>
                        </div>
                    })}
                    {move || success.get().map(|message| view! { <p role="status" class="success">{message}</p> })}
                    {move || error.get().map(|message| view! { <p role="alert" class="error">{message}</p> })}
                    {move || invitation.get().map(|code| view! { <Invitation code /> })}
                    <form class="join-session-form" on:submit=on_join>
                        <h3>"Join with an invitation"</h3>
                        <label for="session-code">"Invitation code"</label>
                        <input id="session-code" required prop:value=move || code.get() on:input=move |ev| set_code.set(event_target_value(&ev)) disabled=move || pending.get() />
                        <button type="submit" disabled=move || pending.get()>"Join session"</button>
                    </form>
                    <details><summary>"Create a session"</summary>
                        <form class="create-session-form" on:submit=on_create>
                            <label for="session-name">"Session name"</label>
                            <input id="session-name" required maxlength="200" prop:value=move || name.get() on:input=move |ev| set_name.set(event_target_value(&ev)) disabled=move || pending.get() />
                            <label for="session-description">"Description (optional)"</label>
                            <textarea id="session-description" rows="2" prop:value=move || description.get() on:input=move |ev| set_description.set(event_target_value(&ev)) disabled=move || pending.get()></textarea>
                            <button type="submit" disabled=move || pending.get()>"Create session"</button>
                        </form>
                    </details>
                    <Show when=move || pending.get()><p role="status">"Saving session..."</p></Show>
                    <h3>"Your sessions"</h3>
                    <Suspense fallback=move || view! { <p role="status">"Loading sessions..."</p> }>
                        {move || sessions.get().map(|result| match &*result {
                            Ok(list) => {
                                let has_next = list.len() > PAGE_SIZE;
                                view! { <div class="session-list">
                                    {list.is_empty().then(|| view! { <p class="empty">"No sessions on this page. Join with an invitation or create a session."</p> })}
                                    {list.iter().take(PAGE_SIZE).cloned().map(|session| view! { <SessionCard session token active_session set_active_session refresh=set_refresh /> }).collect_view()}
                                    <nav class="pagination" aria-label="Session pages">
                                        {(page.get() > 1).then(|| view! { <a href=list_url("/sessions", "code", &code.get(), page.get() - 1)>"Previous"</a> })}
                                        <span>{format!("Page {}", page.get())}</span>
                                        {has_next.then(|| view! { <a href=list_url("/sessions", "code", &code.get(), page.get() + 1)>"Next"</a> })}
                                    </nav>
                                </div> }.into_any()
                            },
                            Err(message) => view! { <div><p role="alert" class="error">{message.clone()}</p><button on:click=move |_| set_refresh.update(|n| *n += 1)>"Retry"</button></div> }.into_any(),
                        })}
                    </Suspense>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn Invitation(code: String) -> impl IntoView {
    let path = format!("/sessions?code={}", query_encode(&code));
    let url = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .map(|origin| format!("{origin}{path}"))
        .unwrap_or(path);
    view! { <p class="session-invitation">"Invitation code: "<strong>{code}</strong>" - "<a href=url.clone()>{url.clone()}</a></p> }
}

#[component]
fn SessionCard(
    session: SessionResponse,
    token: ReadSignal<Option<String>>,
    active_session: ReadSignal<Option<ActiveSession>>,
    set_active_session: WriteSignal<Option<ActiveSession>>,
    refresh: WriteSignal<u32>,
) -> impl IntoView {
    let row = StoredValue::new(session.clone());
    let (pending, set_pending) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (confirm, set_confirm) = signal(None::<String>);
    let (show_results, set_show_results) = signal(false);
    let (result_page, set_result_page) = signal(1usize);
    let (result_refresh, set_result_refresh) = signal(0u32);
    let results = LocalResource::new(move || {
        let auth = token.get();
        let show = show_results.get();
        let page = result_page.get();
        result_refresh.get();
        let id = row.with_value(|row| row.id.clone());
        async move {
            if !show || auth.is_none() {
                return Ok(None);
            }
            let auth = auth.unwrap();
            let detail: SessionResponse =
                get_json(&auth, &format!("/api/tasting-sessions/{id}")).await?;
            let tastings: Vec<SessionTasting> = get_json(
                &auth,
                &format!(
                    "/api/tasting-sessions/{id}/tastings?limit={}&offset={}",
                    PAGE_SIZE + 1,
                    (page - 1) * PAGE_SIZE
                ),
            )
            .await?;
            Ok::<_, String>(Some((detail, tastings, page)))
        }
    });
    let on_select = move |_| {
        if pending.get() {
            return;
        }
        let Some(auth) = token.get() else {
            return;
        };
        set_pending.set(true);
        set_error.set(None);
        let id = row.with_value(|row| row.id.clone());
        leptos::task::spawn_local(async move {
            let result =
                get_json::<SessionResponse>(&auth, &format!("/api/tasting-sessions/{id}")).await;
            if token.try_get_untracked().flatten().as_deref() != Some(&auth) {
                return;
            }
            match result {
                Ok(session) if session.ended_at.is_none() => {
                    set_active_session.set(Some(ActiveSession {
                        id: session.id,
                        name: session.name,
                    }))
                }
                Ok(_) => {
                    set_error.set(Some(
                        "This session has ended. Choose an active session.".into(),
                    ));
                    set_active_session.update(|current| {
                        if current.as_ref().is_some_and(|value| value.id == id) {
                            *current = None;
                        }
                    });
                    refresh.update(|n| *n += 1);
                }
                Err(message) => set_error.set(Some(message)),
            }
            set_pending.set(false);
        });
    };
    let on_confirm = move |_| {
        if pending.get() {
            return;
        }
        let Some(action) = confirm.get() else {
            return;
        };
        let Some(auth) = token.get() else {
            return;
        };
        let id = row.with_value(|row| row.id.clone());
        set_pending.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            let result = post_empty(&auth, &format!("/api/tasting-sessions/{id}/{action}")).await;
            if token.try_get_untracked().flatten().as_deref() != Some(&auth) {
                return;
            }
            match result {
                Ok(()) => {
                    set_active_session.update(|current| {
                        if current.as_ref().is_some_and(|value| value.id == id) {
                            *current = None;
                        }
                    });
                    set_confirm.set(None);
                    refresh.update(|n| *n += 1);
                }
                Err(message) => set_error.set(Some(message)),
            }
            set_pending.set(false);
        });
    };
    let creator = Memo::new(move |_| {
        token
            .get()
            .and_then(|token| token_subject(&token))
            .is_some_and(|id| row.with_value(|row| row.created_by == id))
    });
    view! {
        <article class="session-card" aria-busy=move || pending.get().to_string()>
            <h3>{session.name}</h3>
            <p class="status-badge">{if session.ended_at.is_none() { "Active" } else { "Ended" }}</p>
            {session.description.map(|description| view! { <p>{description}</p> })}
            {session.ended_at.is_none().then(|| view! { <Invitation code=session.join_code /> })}
            {move || error.get().map(|message| view! { <p role="alert" class="error">{message}</p> })}
            {session.ended_at.is_none().then(|| view! {
                <button type="button" disabled=move || pending.get() on:click=on_select>
                    {move || if active_session.get().is_some_and(|selected| row.with_value(|row| selected.id == row.id)) { "Selected for new tastings" } else { "Use for new tastings" }}
                </button>
            })}
            <Show when=move || !creator.get()>
                <button type="button" disabled=move || pending.get() on:click=move |_| { set_confirm.set(Some("leave".into())); set_error.set(None); }>"Leave session"</button>
            </Show>
            <Show when=move || creator.get() && row.with_value(|row| row.ended_at.is_none())>
                <button type="button" disabled=move || pending.get() on:click=move |_| { set_confirm.set(Some("end".into())); set_error.set(None); }>"End session"</button>
            </Show>
            <div hidden=move || confirm.get().is_none()>
                <p>{move || if confirm.get().as_deref() == Some("end") { "End this session for all participants? No more tastings can be added." } else { "Leave this session? It will no longer be available for your new tastings." }}</p>
                <button type="button" disabled=move || pending.get() on:click=on_confirm>"Confirm"</button>
                <button type="button" disabled=move || pending.get() on:click=move |_| set_confirm.set(None)>"Cancel"</button>
            </div>
            <Show when=move || pending.get()><p role="status">"Updating session..."</p></Show>
            <button type="button" aria-expanded=move || show_results.get().to_string() on:click=move |_| set_show_results.update(|show| *show = !*show)>
                {move || if show_results.get() { "Hide participants and results" } else { "Show participants and results" }}
            </button>
            <Show when=move || show_results.get()>
                <Suspense fallback=move || view! { <p role="status">"Loading session results..."</p> }>
                    {move || results.get().map(|result| match &*result {
                        Ok(Some((detail, tastings, loaded_page))) => {
                            let has_next = tastings.len() > PAGE_SIZE;
                            let loaded_page = *loaded_page;
                            view! { <section class="session-results">
                                <h4>"Participants"</h4>
                                <p>{detail.participants.as_ref().map(|participants| participants.iter().map(|participant| participant.username.clone()).collect::<Vec<_>>().join(", ")).filter(|names| !names.is_empty()).unwrap_or_else(|| "Participant details are not available.".into())}</p>
                                <h4>"Tastings"</h4>
                                {tastings.is_empty().then(|| view! { <p>"No tastings on this page."</p> })}
                                <ul>{tastings.iter().take(PAGE_SIZE).map(|tasting| view! { <li><a href=format!("/beers/{}", tasting.beer_id)>{tasting.beer_name.clone().unwrap_or_else(|| "View beer".into())}</a>" - "{format!("{}/10", tasting.score)}</li> }).collect_view()}</ul>
                                <nav aria-label="Session result pages">
                                    <button type="button" disabled=loaded_page <= 1 on:click=move |_| set_result_page.set(loaded_page.saturating_sub(1).max(1))>"Previous results"</button>
                                    <span>{format!("Page {loaded_page}")}</span>
                                    <button type="button" disabled=!has_next on:click=move |_| set_result_page.set(loaded_page + 1)>"Next results"</button>
                                </nav>
                            </section> }.into_any()
                        },
                        Ok(None) => ().into_any(),
                        Err(message) => view! { <div><p role="alert" class="error">{message.clone()}</p><button on:click=move |_| set_result_refresh.update(|n| *n += 1)>"Retry results"</button></div> }.into_any(),
                    })}
                </Suspense>
            </Show>
        </article>
    }
}

// Display logic only. The API authorizes every session mutation.
fn token_subject(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?.replace('-', "+").replace('_', "/");
    let padded = format!("{}{}", payload, "=".repeat((4 - payload.len() % 4) % 4));
    let decoded = web_sys::window()?.atob(&padded).ok()?;
    serde_json::from_str::<serde_json::Value>(&decoded).ok()?["sub"]
        .as_str()
        .map(str::to_string)
}

async fn checked(
    response: Result<gloo_net::http::Response, gloo_net::Error>,
) -> Result<gloo_net::http::Response, String> {
    let response =
        response.map_err(|_| "Could not reach the server. Please try again.".to_string())?;
    if response.ok() {
        return Ok(response);
    }
    let data = response
        .json::<serde_json::Value>()
        .await
        .unwrap_or_default();
    Err(data["error"]
        .as_str()
        .unwrap_or("The session request failed. Please try again.")
        .to_string())
}
async fn get_json<T: DeserializeOwned>(token: &str, url: &str) -> Result<T, String> {
    checked(
        gloo_net::http::Request::get(url)
            .header("Authorization", &format!("Bearer {token}"))
            .send()
            .await,
    )
    .await?
    .json()
    .await
    .map_err(|_| "Could not read the session response.".into())
}
async fn post_json<T: DeserializeOwned>(
    token: &str,
    url: &str,
    body: serde_json::Value,
) -> Result<T, String> {
    checked(
        gloo_net::http::Request::post(url)
            .header("Authorization", &format!("Bearer {token}"))
            .json(&body)
            .map_err(|error| error.to_string())?
            .send()
            .await,
    )
    .await?
    .json()
    .await
    .map_err(|_| "Could not read the session response.".into())
}
async fn post_empty(token: &str, url: &str) -> Result<(), String> {
    checked(
        gloo_net::http::Request::post(url)
            .header("Authorization", &format!("Bearer {token}"))
            .send()
            .await,
    )
    .await
    .map(|_| ())
}
