use leptos::prelude::*;
use serde::Deserialize;

async fn post(path: &str, token: Option<String>) -> Result<serde_json::Value, String> {
    let mut request = gloo_net::http::Request::post(path);
    if let Some(token) = token {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let response = request
        .json(&serde_json::json!({}))
        .map_err(|_| "Could not start verification")?
        .send()
        .await
        .map_err(|_| "Could not reach the server")?;
    let ok = response.ok();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|_| "Invalid server response")?;
    if ok {
        Ok(body)
    } else {
        Err(body["error"]
            .as_str()
            .unwrap_or("Verification failed")
            .to_owned())
    }
}

#[component]
pub fn IdentityButton(
    #[prop(optional)] token: Option<ReadSignal<Option<String>>>,
) -> impl IntoView {
    let enabled = LocalResource::new(|| async {
        let response = gloo_net::http::Request::get("/api/identity")
            .send()
            .await
            .ok()?;
        response
            .json::<serde_json::Value>()
            .await
            .ok()?
            .get("enabled")?
            .as_bool()
    });
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let start = move |_| {
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let tok = token.and_then(|t| t.get_untracked());
            if token.is_some() && tok.is_none() {
                error.set(Some("Sign in before linking your identity.".into()));
                busy.set(false);
                return;
            }
            let path = if token.is_some() {
                "/api/identity/link"
            } else {
                "/api/identity/login"
            };
            match post(path, tok).await {
                Ok(body) => {
                    if let Some(url) = body["url"].as_str()
                        && let Some(window) = web_sys::window()
                    {
                        let _ = window.location().set_href(url);
                    }
                }
                Err(e) => {
                    error.set(Some(e));
                    busy.set(false);
                }
            }
        });
    };
    view! {
        <Show when=move || enabled.get().as_deref()==Some(&Some(true))>
            <button type="button" class="btn btn-secondary" disabled=move || busy.get() on:click=start>
                {move || if busy.get() {"Opening authentik…"} else if token.is_some() {"Link bjoroy.me identity"} else {"Sign in with bjoroy.me"}}
            </button>
            {move || error.get().map(|e|view! {<p class="error" role="alert">{e}</p>})}
        </Show>
    }
}

#[component]
pub fn IdentitySettings(token: ReadSignal<Option<String>>) -> impl IntoView {
    let identity = LocalResource::new(move || {
        let tok = token.get();
        async move {
            let tok = tok.ok_or("Sign in to manage your identity")?;
            let response = gloo_net::http::Request::get("/api/users/me/identity")
                .header("Authorization", &format!("Bearer {tok}"))
                .send()
                .await
                .map_err(|_| "Could not load identity")?;
            if !response.ok() {
                return Err("Could not load identity");
            }
            response
                .json::<serde_json::Value>()
                .await
                .map_err(|_| "Could not load identity")
        }
    });
    view! { <section class="identity-panel" aria-label="bjoroy.me identity">
        <h3>"Your bjoroy.me identity"</h3>
        <p>"Use the same identity service as Sprøyt. We strongly recommend linking now, before temporary test-account email recovery closes. Linking also prepares your account for future social features. Your tastings stay private, and your recovery key and passkeys still work."</p>
        <Suspense fallback=|| view! {<p>"Loading identity…"</p>}>
        {move || identity.get().map(|result| match &*result {
            Ok(data) if data["identity"].is_object() => view! {<p class="success">{format!("Linked to {}",data["identity"]["display_name"].as_str().unwrap_or("bjoroy.me"))}</p>}.into_any(),
            Ok(data) if data["enabled"].as_bool()==Some(true) => view! {<IdentityButton token=token />}.into_any(),
            Ok(_) => view! {<p>"Identity linking is not enabled on this server yet."</p>}.into_any(),
            Err(e) => view! {<p class="error">{e.to_string()}</p>}.into_any(),
        })}
        </Suspense>
    </section> }
}

#[derive(Clone, Deserialize)]
struct Pending {
    linking: bool,
    identity_name: String,
    username: Option<String>,
}
#[component]
pub fn IdentityFinish(token: WriteSignal<Option<String>>) -> impl IntoView {
    let navigate = leptos_router::hooks::use_navigate();
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let pending = LocalResource::new(|| async {
        let response = gloo_net::http::Request::get("/api/identity/pending")
            .send()
            .await
            .map_err(|_| "Could not reach the server")?;
        if !response.ok() {
            return Err(
                "Verification was cancelled, expired or failed. Please return to sign in and try again.",
            );
        }
        response
            .json::<Pending>()
            .await
            .map_err(|_| "Could not load verification")
    });
    let cancel_navigate = navigate.clone();
    let cancel = move |_| {
        busy.set(true);
        let navigate = cancel_navigate.clone();
        leptos::task::spawn_local(async move {
            match post("/api/identity/cancel", None).await {
                Ok(_) => navigate("/login", Default::default()),
                Err(e) => {
                    error.set(Some(e));
                    busy.set(false);
                }
            }
        });
    };
    let confirm = move |_| {
        busy.set(true);
        error.set(None);
        let navigate = navigate.clone();
        leptos::task::spawn_local(async move {
            match post("/api/identity/complete", None).await {
                Ok(body) => {
                    if let Some(jwt) = body["token"].as_str() {
                        super::login::set_autologon(false);
                        if let Some(name) = super::login::extract_username_from_jwt(jwt) {
                            super::login::store_username(&name);
                        }
                        token.set(Some(jwt.into()));
                        navigate("/account", Default::default());
                    }
                }
                Err(e) => {
                    error.set(Some(e));
                    busy.set(false);
                }
            }
        });
    };
    view! { <div class="page"><h2>"Confirm your bjoroy.me identity"</h2>
        <Suspense fallback=||view!{<p>"Checking verification…"</p>}>
        {move || pending.get().map(|result|match &*result {
            Ok(p)=>view!{<p>{if p.linking {format!("Link bjoroy.me identity {} to your Open Tappd account {}?",p.identity_name,p.username.as_deref().unwrap_or(""))}else{format!("Continue signing in as {}?",p.identity_name)}}</p>
                <p>"No tastings or notes are shared with Sprøyt. Recovery sign-in remains available."</p>}.into_any(),
            Err(e)=>view!{<p class="error" role="alert">{e.to_string()}</p>}.into_any()
        })}
        </Suspense>
        <Show when=move || pending.get().is_some_and(|p|p.is_ok())>
            <button class="btn btn-primary" disabled=move ||busy.get() on:click=confirm.clone()>"Confirm and continue"</button>
        </Show>
        {move ||error.get().map(|e|view!{<p class="error" role="alert">{e}</p>})}
        <p><button type="button" class="btn btn-secondary" disabled=move ||busy.get() on:click=cancel>"Cancel and return to sign in"</button></p>
    </div> }
}
