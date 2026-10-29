use leptos::prelude::*;
use serde_json::{Value, json};

async fn post(path: &str, body: Value) -> Result<Value, String> {
    let response = gloo_net::http::Request::post(path)
        .json(&body)
        .map_err(|_| "Could not prepare request")?
        .send()
        .await
        .map_err(|_| "Could not reach the server")?;
    let ok = response.ok();
    let data: Value = response
        .json()
        .await
        .map_err(|_| "Invalid server response")?;
    if ok {
        Ok(data)
    } else {
        Err(data["error"].as_str().unwrap_or("Recovery failed").into())
    }
}
#[component]
pub fn EmailRecoveryPage(token: WriteSignal<Option<String>>) -> impl IntoView {
    let username = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let key = RwSignal::new(None::<String>);
    let info = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let saved = RwSignal::new(false);
    let navigate = leptos_router::hooks::use_navigate();
    let available = LocalResource::new(|| async {
        gloo_net::http::Request::get("/api/email-recovery")
            .send()
            .await
            .ok()?
            .json::<Value>()
            .await
            .ok()
    });
    let request = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match post(
                "/api/email-recovery/request",
                json!({"username":username.get_untracked(),"email":email.get_untracked()}),
            )
            .await
            {
                Ok(data) => info.set(Some(
                    data["message"]
                        .as_str()
                        .unwrap_or("Check your email")
                        .into(),
                )),
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let redeem = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        error.set(None);
        info.set(None);
        leptos::task::spawn_local(async move {
            match post(
                "/api/email-recovery/redeem",
                json!({"username":username.get_untracked(),"code":code.get_untracked()}),
            )
            .await
            {
                Ok(data) => {
                    username.set(data["username"].as_str().unwrap_or_default().into());
                    key.set(data["recovery_key"].as_str().map(str::to_owned));
                    code.set(String::new());
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let login = move |_| {
        if !saved.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        let navigate = navigate.clone();
        leptos::task::spawn_local(async move {
            match post(
                "/api/users/login",
                json!({"username":username.get_untracked(),"recovery_key":key.get_untracked()}),
            )
            .await
            {
                Ok(data) => {
                    if let Some(jwt) = data["token"].as_str() {
                        super::login::set_autologon(false);
                        super::login::store_username(&username.get_untracked());
                        token.set(Some(jwt.into()));
                        key.set(None);
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
    view! {<div class="page auth-page email-recovery"><h2>"Recover your test account"</h2>
        {move ||error.get().map(|e|view!{<p class="error" role="alert">{e}</p>})}
        {move ||info.get().map(|e|view!{<p class="success" role="status">{e}</p>})}
        <Show when=move ||key.get().is_some() fallback=move ||view!{
            <p>"Temporary email recovery is available for existing test accounts with a registered email. New accounts need a recovery key, passkey or linked bjoroy.me identity."</p>
            <Suspense fallback=||view!{<p>"Checking availability…"</p>}>
            {move ||available.get().map(|data|match &*data {
                Some(data) if data["enabled"].as_bool()==Some(true)=>view!{<p class="hint">{format!("This temporary option closes on {}.",data["until"].as_str().unwrap_or("the configured deadline").split('T').next().unwrap_or_default())}</p>}.into_any(),
                _=>view!{<p class="error">"Email recovery is currently closed. Use another sign-in method."</p>}.into_any()
            })}
            </Suspense>
            <form on:submit=request>
                <label for="email-recovery-username">"Username"</label>
                <input id="email-recovery-username" autocomplete="username" required maxlength="30" prop:value=move ||username.get() on:input=move|ev|username.set(event_target_value(&ev)) />
                <label for="email-recovery-address">"Registered email"</label>
                <input id="email-recovery-address" type="email" autocomplete="email" required maxlength="320" prop:value=move ||email.get() on:input=move|ev|email.set(event_target_value(&ev)) />
                <button class="btn-primary" disabled=move ||busy.get() || !available.get().is_some_and(|v|v.as_ref().is_some_and(|d|d["enabled"].as_bool()==Some(true)))>"Send one-time code"</button>
            </form>
            <form on:submit=redeem>
                <h3>"Already received a code?"</h3>
                <p>"Enter your username above and paste the code from your email. It expires after 20 minutes. Using it replaces your recovery key and ends existing login sessions."</p>
                <label for="email-recovery-code">"One-time email code"</label>
                <input id="email-recovery-code" autocomplete="one-time-code" required maxlength="100" prop:value=move ||code.get() on:input=move|ev|code.set(event_target_value(&ev)) />
                <button class="btn-primary" disabled=move ||busy.get() || username.get().trim().is_empty()>"Get a new recovery key"</button>
            </form>
        }>
            <div class="recovery-key-reveal">
                <h3>"Save your new recovery key"</h3><p>"Your old key no longer works. This new key is shown only here; save it before continuing."</p>
                <p class="recovery-key-value">{move ||key.get()}</p>
                <label><input type="checkbox" prop:checked=move||saved.get() on:change=move|ev|saved.set(event_target_checked(&ev)) />" I have saved my new recovery key"</label>
                <p>"Next, link your bjoroy.me identity from My Profile. It is the recommended way to keep access when temporary email recovery closes."</p>
                <button class="btn-primary" on:click=login.clone() disabled=move ||busy.get() || !saved.get()>"Continue to My Profile"</button>
            </div>
        </Show>
        <p><a href="/login">"Back to sign in"</a></p>
    </div>}
}
