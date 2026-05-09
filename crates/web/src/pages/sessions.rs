use leptos::prelude::*;
use serde::Deserialize;

use crate::pages::rate_beer::ActiveSession;

#[derive(Debug, Clone, Deserialize)]
struct SessionResponse {
    id: String,
    name: String,
    description: Option<String>,
    join_code: String,
    visibility: String,
    started_at: String,
    ended_at: Option<String>,
    auto_end_minutes: i32,
    participants: Option<Vec<ParticipantInfo>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ParticipantInfo {
    username: String,
}

#[component]
pub fn SessionBrowserPage(
    token: ReadSignal<Option<String>>,
    active_session: ReadSignal<Option<ActiveSession>>,
    set_active_session: WriteSignal<Option<ActiveSession>>,
) -> impl IntoView {
    let (sessions, set_sessions) = signal(Vec::<SessionResponse>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<String>::None);

    // Create form
    let (show_create, set_show_create) = signal(false);
    let (new_name, set_new_name) = signal(String::new());
    let (new_desc, set_new_desc) = signal(String::new());
    let (creating, set_creating) = signal(false);

    // Join by code
    let (join_code, set_join_code) = signal(String::new());
    let (joining, set_joining) = signal(false);

    // Load sessions
    let load = move || {
        let tok = token.get();
        if let Some(tok) = tok {
            set_loading.set(true);
            leptos::task::spawn_local(async move {
                match fetch_sessions(&tok).await {
                    Ok(list) => set_sessions.set(list),
                    Err(e) => set_error.set(Some(e)),
                }
                set_loading.set(false);
            });
        }
    };

    // Initial load
    {
        let load = load.clone();
        leptos::task::spawn_local(async move { load() });
    }

    let on_create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        let name = new_name.get();
        if name.is_empty() {
            set_error.set(Some("Session name is required".into()));
            return;
        }

        set_creating.set(true);
        set_error.set(None);
        let desc = new_desc.get();

        leptos::task::spawn_local(async move {
            match create_session(&tok, &name, if desc.is_empty() { None } else { Some(&desc) }).await {
                Ok(session) => {
                    set_success.set(Some(format!("Session '{}' created! Join code: {}", session.name, session.join_code)));
                    set_active_session.set(Some(ActiveSession {
                        id: session.id.clone(),
                        name: session.name.clone(),
                    }));
                    set_new_name.set(String::new());
                    set_new_desc.set(String::new());
                    set_show_create.set(false);
                    load();
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_creating.set(false);
        });
    };

    let on_join_code = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        let code = join_code.get();
        if code.is_empty() {
            set_error.set(Some("Enter a join code".into()));
            return;
        }

        set_joining.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            match join_by_code(&tok, &code).await {
                Ok(session) => {
                    set_success.set(Some(format!("Joined '{}'!", session.name)));
                    set_active_session.set(Some(ActiveSession {
                        id: session.id.clone(),
                        name: session.name.clone(),
                    }));
                    set_join_code.set(String::new());
                    load();
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_joining.set(false);
        });
    };

    view! {
        <div class="page sessions">
            <h2>"Tasting Sessions 🥂"</h2>

            {move || {
                if token.get().is_none() {
                    Some(view! { <p class="login-prompt">"Log in to view and join sessions"</p> })
                } else {
                    None
                }
            }}

            {move || success.get().map(|msg| view! { <div class="success">{msg}</div> })}
            {move || error.get().map(|e| view! { <div class="error">{e}</div> })}

            {move || active_session.get().map(|s| view! {
                <div class="active-session-banner">
                    "📋 Active session: " <strong>{s.name.clone()}</strong>
                    <button
                        class="btn-text"
                        on:click=move |_| set_active_session.set(None)
                    >"Leave"</button>
                </div>
            })}

            <div class="session-actions" style:display=move || if token.get().is_some() { "flex" } else { "none" }>
                <button class="btn-primary" on:click=move |_| set_show_create.update(|v| *v = !*v)>
                    {move || if show_create.get() { "Cancel" } else { "+ New Session" }}
                </button>

                <form class="join-code-form" on:submit=on_join_code>
                    <input
                        type="text"
                        placeholder="Join code..."
                        maxlength="6"
                        prop:value=move || join_code.get()
                        on:input=move |ev| set_join_code.set(event_target_value(&ev).to_uppercase())
                    />
                    <button type="submit" disabled=move || joining.get()>
                        {move || if joining.get() { "..." } else { "Join" }}
                    </button>
                </form>
            </div>

            {move || show_create.get().then(|| view! {
                <form class="create-session-form" on:submit=on_create>
                    <div class="form-group">
                        <label>"Session Name"</label>
                        <input
                            type="text"
                            required=true
                            placeholder="Friday Night Tasting..."
                            prop:value=move || new_name.get()
                            on:input=move |ev| set_new_name.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-group">
                        <label>"Description (optional)"</label>
                        <textarea
                            rows="2"
                            placeholder="What's this session about?"
                            prop:value=move || new_desc.get()
                            on:input=move |ev| set_new_desc.set(event_target_value(&ev))
                        ></textarea>
                    </div>
                    <button type="submit" class="btn-primary" disabled=move || creating.get()>
                        {move || if creating.get() { "Creating..." } else { "Create Session" }}
                    </button>
                </form>
            })}

            <div class="session-list">
                {move || {
                    if loading.get() {
                        return view! { <p>"Loading sessions..."</p> }.into_any();
                    }
                    let list = sessions.get();
                    if list.is_empty() {
                        return view! { <p class="empty">"No sessions yet. Create one or join with a code!"</p> }.into_any();
                    }
                    view! {
                        <div class="session-grid">
                            {list.iter().map(|s| {
                                let is_active = s.ended_at.is_none();
                                let participants = s.participants.as_ref()
                                    .map(|p| p.iter().map(|x| x.username.clone()).collect::<Vec<_>>().join(", "))
                                    .unwrap_or_default();
                                let sid = s.id.clone();
                                let sname = s.name.clone();
                                let tok_join = token.get();
                                view! {
                                    <div class=if is_active { "session-card active" } else { "session-card ended" }>
                                        <div class="session-card-header">
                                            <h3>{s.name.clone()}</h3>
                                            {if is_active {
                                                view! { <span class="status-badge active">"Active"</span> }.into_any()
                                            } else {
                                                view! { <span class="status-badge ended">"Ended"</span> }.into_any()
                                            }}
                                        </div>
                                        {s.description.as_ref().map(|d| view! { <p class="session-desc">{d.clone()}</p> })}
                                        <p class="session-meta">
                                            "Code: " <code>{s.join_code.clone()}</code>
                                            " • " {s.visibility.clone()}
                                        </p>
                                        {(!participants.is_empty()).then(|| view! {
                                            <p class="session-participants">"👥 " {participants}</p>
                                        })}
                                        {is_active.then(|| {
                                            let sid2 = sid.clone();
                                            let sname2 = sname.clone();
                                            view! {
                                                <button
                                                    class="btn-secondary"
                                                    on:click=move |_| {
                                                        set_active_session.set(Some(ActiveSession {
                                                            id: sid2.clone(),
                                                            name: sname2.clone(),
                                                        }));
                                                        set_success.set(Some(format!("Now in session '{}'", sname2)));
                                                    }
                                                >"Set Active"</button>
                                            }
                                        })}
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                }}
            </div>
        </div>
    }
}

async fn fetch_sessions(token: &str) -> Result<Vec<SessionResponse>, String> {
    let resp = gloo_net::http::Request::get("/api/tasting-sessions")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<SessionResponse>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to fetch sessions".to_string())
    }
}

async fn create_session(
    token: &str,
    name: &str,
    description: Option<&str>,
) -> Result<SessionResponse, String> {
    let mut body = serde_json::json!({ "name": name });
    if let Some(d) = description {
        body["description"] = serde_json::Value::String(d.to_string());
    }

    let resp = gloo_net::http::Request::post("/api/tasting-sessions")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<SessionResponse>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(data["error"].as_str().unwrap_or("Failed to create session").to_string())
    }
}

async fn join_by_code(token: &str, code: &str) -> Result<SessionResponse, String> {
    let body = serde_json::json!({ "code": code });

    let resp = gloo_net::http::Request::post("/api/tasting-sessions/join")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<SessionResponse>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(data["error"].as_str().unwrap_or("Invalid join code").to_string())
    }
}
