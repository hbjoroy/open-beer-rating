use leptos::prelude::*;

#[component]
pub fn RegisterPage(on_success: impl Fn() + Send + 'static) -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(false);
    let (loading, set_loading) = signal(false);
    let on_success = std::sync::Arc::new(on_success);

    let on_submit = {
        let on_success = on_success.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            set_loading.set(true);
            set_error.set(None);

            let username_val = username.get();
            let password_val = password.get();
            let email_val = email.get();
            let email_opt = if email_val.is_empty() {
                None
            } else {
                Some(email_val)
            };
            let on_success = on_success.clone();

            leptos::task::spawn_local(async move {
                match do_register(&username_val, &password_val, email_opt.as_deref()).await {
                    Ok(()) => {
                        set_success.set(true);
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
            <h2>"Register"</h2>
            <p class="privacy-note">"Email is optional. You can use Open Tappd pseudonymously."</p>

            {move || success.get().then(|| view! {
                <p class="success">"Account created! You can now log in."</p>
            })}

            <form on:submit=on_submit style:display=move || if success.get() { "none" } else { "block" }>
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
                    <label for="password">"Password (min 8 characters)"</label>
                    <input
                        id="password"
                        type="password"
                        prop:value=move || password.get()
                        on:input=move |ev| set_password.set(event_target_value(&ev))
                        required
                    />
                </div>
                <div class="form-group">
                    <label for="email">"Email (optional — for password recovery only)"</label>
                    <input
                        id="email"
                        type="email"
                        prop:value=move || email.get()
                        on:input=move |ev| set_email.set(event_target_value(&ev))
                    />
                </div>
                {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
                <button type="submit" disabled=move || loading.get()>
                    {move || if loading.get() { "Creating account..." } else { "Register" }}
                </button>
            </form>
        </div>
    }
}

async fn do_register(username: &str, password: &str, email: Option<&str>) -> Result<(), String> {
    let mut body = serde_json::json!({
        "username": username,
        "password": password,
    });
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
        Ok(())
    } else {
        let data: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::json!({"error": "Registration failed"}));
        Err(data["error"]
            .as_str()
            .unwrap_or("Registration failed")
            .to_string())
    }
}
