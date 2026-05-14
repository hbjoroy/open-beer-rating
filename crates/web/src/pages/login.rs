use leptos::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoginMode {
    Choose,
    NewDevice,
    RecoveryOnly,
}

#[component]
pub fn LoginPage(
    token: WriteSignal<Option<String>>,
    on_success: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let (error, set_error) = signal(Option::<String>::None);
    let (info, set_info) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);
    let (mode, set_mode) = signal(LoginMode::Choose);
    let (username, set_username) = signal(get_stored_username().unwrap_or_default());
    let (recovery_key, set_recovery_key) = signal(String::new());
    let on_success = Arc::new(on_success);

    let on_passkey_login = {
        let on_success = on_success.clone();
        move |_| {
            set_loading.set(true);
            set_error.set(None);
            let on_success = on_success.clone();

            leptos::task::spawn_local(async move {
                match do_passkey_login().await {
                    Ok(tok) => {
                        // Store username from JWT for future login hints
                        if let Some(name) = extract_username_from_jwt(&tok) {
                            store_username(&name);
                        }
                        token.set(Some(tok));
                        on_success();
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                        set_loading.set(false);
                    }
                }
            });
        }
    };

    let on_new_device_submit = {
        let on_success = on_success.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            set_loading.set(true);
            set_error.set(None);
            set_info.set(None);

            let username_val = username.get();
            let recovery_val = recovery_key.get();
            let on_success = on_success.clone();

            leptos::task::spawn_local(async move {
                match do_recovery_login(&username_val, &recovery_val).await {
                    Ok(tok) => {
                        store_username(&username_val);
                        set_info.set(Some("✅ Signed in! Setting up passkey on this device...".into()));
                        match do_passkey_register_after_recovery(&tok).await {
                            Ok(()) => {
                                set_info.set(Some("✅ Passkey registered on this device!".into()));
                            }
                            Err(e) => {
                                set_info.set(Some(format!("⚠️ Signed in, but passkey setup skipped: {e}")));
                            }
                        }
                        token.set(Some(tok));
                        on_success();
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                        set_loading.set(false);
                    }
                }
            });
        }
    };

    let on_recovery_submit = {
        let on_success = on_success.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            set_loading.set(true);
            set_error.set(None);

            let username_val = username.get();
            let recovery_val = recovery_key.get();
            let on_success = on_success.clone();

            leptos::task::spawn_local(async move {
                match do_recovery_login(&username_val, &recovery_val).await {
                    Ok(tok) => {
                        store_username(&username_val);
                        token.set(Some(tok));
                        on_success();
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                        set_loading.set(false);
                    }
                }
            });
        }
    };

    view! {
        <div class="page auth-page">
            <h2>"Sign In"</h2>

            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
            {move || info.get().map(|i| view! { <p class="success">{i}</p> })}

            {move || match mode.get() {
                LoginMode::Choose => view! {
                    <div class="login-options">
                        <button
                            class="btn-primary btn-large"
                            on:click=on_passkey_login.clone()
                            disabled=move || loading.get()
                        >
                            {move || if loading.get() { "Signing in..." } else { "🔑 Sign in with Passkey" }}
                        </button>
                        <p class="hint">"Use your fingerprint, face, or device PIN"</p>

                        <div class="divider"><span>"or"</span></div>

                        <div class="login-alt-options">
                            <button class="btn-secondary" on:click=move |_| set_mode.set(LoginMode::NewDevice)>
                                "📱 New Device"
                            </button>
                            <p class="hint">"Sign in with recovery key and set up a passkey on this device"</p>

                            <button class="btn-text" on:click=move |_| set_mode.set(LoginMode::RecoveryOnly)>
                                "🔐 Recovery Key Only"
                            </button>
                        </div>
                    </div>
                }.into_any(),

                LoginMode::NewDevice => view! {
                    <div class="recovery-section">
                        <h3>"📱 New Device Setup"</h3>
                        <p class="hint">"Sign in with your recovery key, then we'll register a passkey on this device so you can use it next time."</p>

                        <form on:submit=on_new_device_submit.clone()>
                            <div class="form-group">
                                <label for="username">"Username"</label>
                                <input
                                    id="username"
                                    type="text"
                                    prop:value=move || username.get()
                                    on:input=move |ev| set_username.set(event_target_value(&ev))
                                    required
                                />
                            </div>
                            <div class="form-group">
                                <label for="recovery-key">"Recovery Key"</label>
                                <input
                                    id="recovery-key"
                                    type="text"
                                    prop:value=move || recovery_key.get()
                                    on:input=move |ev| set_recovery_key.set(event_target_value(&ev))
                                    placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
                                    required
                                />
                            </div>
                            <button type="submit" disabled=move || loading.get()>
                                {move || if loading.get() { "Setting up..." } else { "Sign in & Set Up Passkey" }}
                            </button>
                        </form>

                        <button class="btn-text" on:click=move |_| { set_mode.set(LoginMode::Choose); set_error.set(None); }>
                            "← Back"
                        </button>
                    </div>
                }.into_any(),

                LoginMode::RecoveryOnly => view! {
                    <div class="recovery-section">
                        <h3>"🔐 Recovery Key Sign In"</h3>
                        <p class="hint">"Sign in using your recovery key only. No passkey will be created on this device."</p>

                        <form on:submit=on_recovery_submit.clone()>
                            <div class="form-group">
                                <label for="username">"Username"</label>
                                <input
                                    id="username"
                                    type="text"
                                    prop:value=move || username.get()
                                    on:input=move |ev| set_username.set(event_target_value(&ev))
                                    required
                                />
                            </div>
                            <div class="form-group">
                                <label for="recovery-key">"Recovery Key"</label>
                                <input
                                    id="recovery-key"
                                    type="text"
                                    prop:value=move || recovery_key.get()
                                    on:input=move |ev| set_recovery_key.set(event_target_value(&ev))
                                    placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
                                    required
                                />
                            </div>
                            <button type="submit" disabled=move || loading.get()>
                                {move || if loading.get() { "Signing in..." } else { "Sign In" }}
                            </button>
                        </form>

                        <button class="btn-text" on:click=move |_| { set_mode.set(LoginMode::Choose); set_error.set(None); }>
                            "← Back"
                        </button>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

async fn do_passkey_login() -> Result<String, String> {
    // Step 1: Get challenge
    let resp = gloo_net::http::Request::post("/api/passkeys/auth/start")
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !resp.ok() {
        return Err("Failed to start passkey auth".into());
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    let challenge_options = &data["challenge"];

    // Step 2: Call navigator.credentials.get()
    let options_json = challenge_options.to_string();
    let credential_json = call_credentials_get(&options_json).await?;

    // Step 3: Send to server
    let resp = gloo_net::http::Request::post("/api/passkeys/auth/finish")
        .header("Content-Type", "application/json")
        .body(credential_json)
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
        data["token"].as_str().map(|s| s.to_string()).ok_or("No token".into())
    } else {
        let data: serde_json::Value = resp.json().await
            .unwrap_or(serde_json::json!({"error": "Authentication failed"}));
        Err(data["error"].as_str().unwrap_or("Authentication failed").to_string())
    }
}

async fn do_recovery_login(username: &str, recovery_key: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "username": username,
        "recovery_key": recovery_key,
    });

    let resp = gloo_net::http::Request::post("/api/users/login")
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
        data["token"].as_str().map(|s| s.to_string()).ok_or("No token".into())
    } else {
        let data: serde_json::Value = resp.json().await
            .unwrap_or(serde_json::json!({"error": "Login failed"}));
        Err(data["error"].as_str().unwrap_or("Login failed").to_string())
    }
}

async fn do_passkey_register_after_recovery(jwt_token: &str) -> Result<(), String> {
    // Same flow as registration passkey setup
    let resp = gloo_net::http::Request::post("/api/passkeys/register/start")
        .header("Authorization", &format!("Bearer {jwt_token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !resp.ok() {
        return Err("Failed to start passkey registration".into());
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    let options_json = data["challenge"].to_string();
    let credential_json = call_credentials_create(&options_json).await?;

    let resp = gloo_net::http::Request::post("/api/passkeys/register/finish")
        .header("Authorization", &format!("Bearer {jwt_token}"))
        .header("Content-Type", "application/json")
        .body(credential_json)
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() { Ok(()) } else { Err("Failed to register passkey".into()) }
}

async fn call_credentials_get(options_json: &str) -> Result<String, String> {
    use js_sys::Function;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;

    let js_code = format!(
        r#"
        (async function() {{
            const options = {options_json};

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
            if (options.allowCredentials) {{
                options.allowCredentials = options.allowCredentials.map(c => ({{
                    ...c, id: b64urlToBuffer(c.id)
                }}));
            }}

            const credential = await navigator.credentials.get({{ publicKey: options }});

            return JSON.stringify({{
                id: credential.id,
                rawId: bufferToB64url(credential.rawId),
                type: credential.type,
                response: {{
                    authenticatorData: bufferToB64url(credential.response.authenticatorData),
                    clientDataJSON: bufferToB64url(credential.response.clientDataJSON),
                    signature: bufferToB64url(credential.response.signature),
                    userHandle: credential.response.userHandle ? Array.from(new Uint8Array(credential.response.userHandle)) : null
                }}
            }});
        }})()
        "#
    );

    let eval_fn = Function::new_no_args(&format!("return {}", js_code.trim()));
    let promise = eval_fn.call0(&JsValue::NULL).map_err(|e| format!("JS error: {e:?}"))?;
    let result = JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| crate::components::webauthn_errors::friendly_webauthn_error(&format!("{e:?}")))?;

    result.as_string().ok_or("No result from credentials.get".into())
}

async fn call_credentials_create(options_json: &str) -> Result<String, String> {
    use js_sys::Function;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;

    let js_code = format!(
        r#"
        (async function() {{
            const options = {options_json};

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

            const credential = await navigator.credentials.create({{ publicKey: options }});

            return JSON.stringify({{
                id: credential.id,
                rawId: bufferToB64url(credential.rawId),
                type: credential.type,
                response: {{
                    clientDataJSON: bufferToB64url(credential.response.clientDataJSON),
                    attestationObject: bufferToB64url(credential.response.attestationObject),
                    transports: credential.response.getTransports ? credential.response.getTransports() : []
                }}
            }});
        }})()
        "#
    );

    let eval_fn = Function::new_no_args(&format!("return {}", js_code.trim()));
    let promise = eval_fn.call0(&JsValue::NULL).map_err(|e| format!("JS error: {e:?}"))?;
    let result = JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| crate::components::webauthn_errors::friendly_webauthn_error(&format!("{e:?}")))?;

    result.as_string().ok_or("No result from credentials.create".into())
}

/// Read stored username from localStorage.
fn get_stored_username() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("open_tappd_username").ok())
        .flatten()
        .filter(|u| !u.is_empty())
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

/// Extract username from a JWT token (base64-decode the payload).
fn extract_username_from_jwt(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let payload = parts[1];
    let padded = match payload.len() % 4 {
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => payload.to_string(),
    };
    let b64 = padded.replace('-', "+").replace('_', "/");
    let window = web_sys::window()?;
    let decoded = window.atob(&b64).ok()?;
    let bytes: Vec<u8> = decoded.chars().map(|c| c as u8).collect();
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value["username"].as_str().map(|s| s.to_string())
}
