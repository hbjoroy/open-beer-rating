use leptos::prelude::*;
use std::sync::Arc;

#[component]
pub fn RegisterPage(
    token: WriteSignal<Option<String>>,
    on_success: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (phase, set_phase) = signal(Phase::Form);
    let (recovery_key, set_recovery_key) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let on_success = Arc::new(on_success);

    let on_submit = {
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            set_loading.set(true);
            set_error.set(None);

            let username_val = username.get();
            let email_val = email.get();
            let email_opt = if email_val.is_empty() { None } else { Some(email_val) };

            leptos::task::spawn_local(async move {
                match do_register(&username_val, email_opt.as_deref()).await {
                    Ok(reg_result) => {
                        // Try passkey registration — required, not optional
                        match do_passkey_register(&reg_result.token).await {
                            Ok(()) => {
                                // Passkey created — show recovery key
                                set_recovery_key.set(reg_result.recovery_key.clone());
                                set_phase.set(Phase::ShowRecoveryKey);
                                token.set(Some(reg_result.token));

                                // Remember username and enable autologon
                                store_username(&username_val);
                                crate::pages::login::set_autologon(true);
                            }
                            Err(e) => {
                                // Passkey failed/cancelled — clean up orphaned account
                                let _ = abort_registration(&reg_result.token).await;
                                set_error.set(Some(format!(
                                    "Passkey registration failed: {e}. Please try again — a passkey is required."
                                )));
                            }
                        }
                        set_loading.set(false);
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                        set_loading.set(false);
                    }
                }
            });
        }
    };

    let on_confirm_saved = {
        let on_success = on_success.clone();
        move |_| {
            on_success();
        }
    };

    view! {
        <div class="page auth-page">
            <h2>"Create Account"</h2>

            {move || match phase.get() {
                Phase::Form => view! {
                    <div>
                        <p class="privacy-note">"Email is optional. You can use Open Tappd pseudonymously."</p>
                        <p class="privacy-note">"A passkey (fingerprint/face/PIN) will be your login method."</p>

                        <form on:submit=on_submit.clone()>
                            <div class="form-group">
                                <label for="username">"Username"</label>
                                <input
                                    id="username"
                                    type="text"
                                    prop:value=move || username.get()
                                    on:input=move |ev| set_username.set(event_target_value(&ev))
                                    placeholder="3-30 chars, letters/numbers/_/-"
                                    required
                                />
                            </div>
                            <div class="form-group">
                                <label for="email">"Email (optional)"</label>
                                <input
                                    id="email"
                                    type="email"
                                    prop:value=move || email.get()
                                    on:input=move |ev| set_email.set(event_target_value(&ev))
                                />
                            </div>
                            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
                            <button type="submit" disabled=move || loading.get()>
                                {move || if loading.get() { "Creating account..." } else { "Register with Passkey" }}
                            </button>
                        </form>
                    </div>
                }.into_any(),

                Phase::ShowRecoveryKey => view! {
                    <div class="recovery-key-reveal">
                        <h3>"✅ Account Created!"</h3>

                        {move || error.get().map(|e| view! { <p class="warning">{e}</p> })}

                        <div class="recovery-key-box">
                            <p class="recovery-key-label">"Your Recovery Key — save this somewhere safe!"</p>
                            <p class="recovery-key-value">{move || recovery_key.get()}</p>
                            <p class="recovery-key-warning">
                                "⚠️ This is shown only once. If you lose your passkey and this key, "
                                "you will lose access to your account permanently."
                            </p>
                        </div>

                        <button class="btn-primary" on:click=on_confirm_saved.clone()>
                            "I've saved my recovery key"
                        </button>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    Form,
    ShowRecoveryKey,
}

#[derive(Debug, serde::Deserialize)]
struct RegisterResult {
    token: String,
    recovery_key: String,
}

async fn do_register(username: &str, email: Option<&str>) -> Result<RegisterResult, String> {
    let mut body = serde_json::json!({ "username": username });
    if let Some(email) = email {
        body["email"] = serde_json::Value::String(email.to_string());
    }

    let resp = gloo_net::http::Request::post("/api/users/register")
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
        Ok(RegisterResult {
            token: data["token"].as_str().ok_or("No token")?.to_string(),
            recovery_key: data["recovery_key"].as_str().ok_or("No recovery key")?.to_string(),
        })
    } else {
        let data: serde_json::Value = resp.json().await
            .unwrap_or(serde_json::json!({"error": "Registration failed"}));
        Err(data["error"].as_str().unwrap_or("Registration failed").to_string())
    }
}

async fn do_passkey_register(jwt_token: &str) -> Result<(), String> {
    // Step 1: Get challenge from server
    web_sys::console::log_1(&"[passkey] Starting registration...".into());
    let resp = gloo_net::http::Request::post("/api/passkeys/register/start")
        .header("Authorization", &format!("Bearer {jwt_token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Failed to start passkey registration (HTTP {status}): {body}"));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    let challenge_options = &data["challenge"];

    // Log the full server response for debugging
    web_sys::console::log_1(&format!("[passkey] Server challenge response: {}", serde_json::to_string_pretty(challenge_options).unwrap_or_default()).into());

    // Step 2: Call navigator.credentials.create() via JS interop
    let options_json = challenge_options.to_string();
    let credential_json = call_credentials_create(&options_json).await?;

    web_sys::console::log_1(&"[passkey] Credential created, sending to server...".into());

    // Step 3: Send credential to server
    let resp = gloo_net::http::Request::post("/api/passkeys/register/finish")
        .header("Authorization", &format!("Bearer {jwt_token}"))
        .header("Content-Type", "application/json")
        .body(credential_json)
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        web_sys::console::log_1(&"[passkey] Registration complete!".into());
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(format!("Failed to complete passkey registration (HTTP {status}): {body}"))
    }
}

async fn call_credentials_create(options_json: &str) -> Result<String, String> {
    use js_sys::Function;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;

    // Use eval-based approach to call the WebAuthn API
    // This is the pragmatic WASM approach since web-sys PublicKeyCredential
    // bindings require extensive manual ArrayBuffer conversion
    let js_code = format!(
        r#"
        (async function() {{
            const options = {options_json};

            console.log('[passkey] Raw options from server:', JSON.stringify(options, null, 2));
            console.log('[passkey] rp.id:', options.rp?.id);
            console.log('[passkey] rp.name:', options.rp?.name);
            console.log('[passkey] Current origin:', window.location.origin);
            console.log('[passkey] Current hostname:', window.location.hostname);

            // Decode base64url fields to ArrayBuffers
            function b64urlToBuffer(b64) {{
                const padding = '='.repeat((4 - b64.length % 4) % 4);
                const base64 = (b64 + padding).replace(/-/g, '+').replace(/_/g, '/');
                const rawData = atob(base64);
                const buffer = new Uint8Array(rawData.length);
                for (let i = 0; i < rawData.length; i++) buffer[i] = rawData.charCodeAt(i);
                return buffer.buffer;
            }}

            function bufferToB64url(buffer) {{
                const bytes = new Uint8Array(buffer);
                let binary = '';
                for (let i = 0; i < bytes.byteLength; i++) binary += String.fromCharCode(bytes[i]);
                return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
            }}

            options.challenge = b64urlToBuffer(options.challenge);
            options.user.id = b64urlToBuffer(options.user.id);
            if (options.excludeCredentials) {{
                options.excludeCredentials = options.excludeCredentials.map(c => ({{
                    ...c, id: b64urlToBuffer(c.id)
                }}));
            }}

            console.log('[passkey] Final publicKey options for credentials.create():', JSON.parse(JSON.stringify(options, (k, v) => v instanceof ArrayBuffer ? '[ArrayBuffer ' + v.byteLength + ' bytes]' : v)));

            const credential = await navigator.credentials.create({{ publicKey: options }});

            const result = JSON.stringify({{
                id: credential.id,
                rawId: bufferToB64url(credential.rawId),
                type: credential.type,
                response: {{
                    clientDataJSON: bufferToB64url(credential.response.clientDataJSON),
                    attestationObject: bufferToB64url(credential.response.attestationObject),
                    transports: credential.response.getTransports ? credential.response.getTransports() : []
                }}
            }});

            console.log('[passkey] Credential response to send to server:', result);

            return result;
        }})()
        "#
    );

    // ASI fix: "return\n(...)" is parsed as "return;" — must be on same line
    let eval_fn = Function::new_no_args(&format!("return {}", js_code.trim()));
    let promise = eval_fn.call0(&JsValue::NULL).map_err(|e| format!("JS error: {e:?}"))?;
    let result = JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| crate::components::webauthn_errors::friendly_webauthn_error(&format!("{e:?}")))?;

    result.as_string().ok_or("No result from credentials.create".into())
}

/// Store username in localStorage for future passkey login hints.
fn store_username(username: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("open_tappd_username", username);
    }
}

/// Call the server to delete the orphaned account after failed passkey registration.
async fn abort_registration(jwt_token: &str) -> Result<(), String> {
    let _ = gloo_net::http::Request::post("/api/users/register/abort")
        .header("Authorization", &format!("Bearer {jwt_token}"))
        .send()
        .await;
    Ok(())
}
