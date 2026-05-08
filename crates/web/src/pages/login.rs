use leptos::prelude::*;

#[component]
pub fn LoginPage(
    token: WriteSignal<Option<String>>,
    on_success: impl Fn() + Send + 'static,
) -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
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
            let on_success = on_success.clone();

            leptos::task::spawn_local(async move {
                match do_login(&username_val, &password_val).await {
                    Ok(tok) => {
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
            <h2>"Login"</h2>
            <form on:submit=on_submit>
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
                    <label for="password">"Password"</label>
                    <input
                        id="password"
                        type="password"
                        prop:value=move || password.get()
                        on:input=move |ev| set_password.set(event_target_value(&ev))
                        required
                    />
                </div>
                {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
                <button type="submit" disabled=move || loading.get()>
                    {move || if loading.get() { "Logging in..." } else { "Login" }}
                </button>
            </form>
        </div>
    }
}

async fn do_login(username: &str, password: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "username": username,
        "password": password,
    });

    let resp = gloo_net::http::Request::post("/api/users/login")
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {e}"))?;
        data["token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No token in response".to_string())
    } else {
        let data: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::json!({"error": "Login failed"}));
        Err(data["error"].as_str().unwrap_or("Login failed").to_string())
    }
}
