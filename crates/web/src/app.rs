use leptos::prelude::*;

use crate::pages;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Page {
    Home,
    Login,
    Register,
    BeerList,
    BeerDetail(String),
    AddBeer,
    Profile,
}

#[component]
pub fn App() -> impl IntoView {
    let (current_page, set_page) = signal(Page::Home);
    let (token, set_token) = signal(Option::<String>::None);

    // Silent passkey auto-auth on mount
    {
        let set_token = set_token;
        leptos::task::spawn_local(async move {
            if let Ok(tok) = try_silent_passkey_auth().await {
                set_token.set(Some(tok));
            }
        });
    }

    let nav_class = "nav-link";

    view! {
        <div class="app">
            <header class="header">
                <h1 class="logo">"🍺 Open Tappd"</h1>
                <nav class="nav">
                    <a class=nav_class on:click=move |_| set_page.set(Page::Home)>"Home"</a>
                    <a class=nav_class on:click=move |_| set_page.set(Page::BeerList)>"Beers"</a>
                    {move || {
                        if token.get().is_some() {
                            view! {
                                <a class=nav_class on:click=move |_| set_page.set(Page::AddBeer)>"+ Add Beer"</a>
                                <a class=nav_class on:click=move |_| set_page.set(Page::Profile)>"Profile"</a>
                                <a class=nav_class on:click=move |_| set_token.set(None)>"Logout"</a>
                            }.into_any()
                        } else {
                            view! {
                                <a class=nav_class on:click=move |_| set_page.set(Page::Login)>"Login"</a>
                                <a class=nav_class on:click=move |_| set_page.set(Page::Register)>"Register"</a>
                            }.into_any()
                        }
                    }}
                </nav>
            </header>

            <main class="content">
                {move || {
                    let page = current_page.get();
                    match page {
                        Page::Home => view! { <pages::home::HomePage /> }.into_any(),
                        Page::Login => view! { <pages::login::LoginPage token=set_token on_success=move || set_page.set(Page::BeerList) /> }.into_any(),
                        Page::Register => view! { <pages::register::RegisterPage token=set_token on_success=move || set_page.set(Page::BeerList) /> }.into_any(),
                        Page::BeerList => view! {
                            <pages::beer_list::BeerListPage
                                token=token
                                on_view_beer=move |id: String| set_page.set(Page::BeerDetail(id))
                                on_add_beer=move || set_page.set(Page::AddBeer)
                            />
                        }.into_any(),
                        Page::BeerDetail(id) => view! {
                            <pages::beer_detail::BeerDetailPage
                                beer_id=id
                                token=token
                                on_back=move || set_page.set(Page::BeerList)
                            />
                        }.into_any(),
                        Page::AddBeer => view! {
                            <pages::add_beer::AddBeerPage token=token />
                        }.into_any(),
                        Page::Profile => view! { <pages::profile::ProfilePage token=token /> }.into_any(),
                    }
                }}
            </main>

            <footer class="footer">
                <p>"Open Tappd — Community-owned, privacy-first beer tasting 🍺"</p>
                <p class="privacy-note">"Your data is yours. Private by default."</p>
            </footer>
        </div>
    }
}

async fn try_silent_passkey_auth() -> Result<String, String> {
    use js_sys::Function;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;

    // Step 1: Get auth challenge
    let resp = gloo_net::http::Request::post("/api/passkeys/auth/start")
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !resp.ok() {
        return Err("Auth start failed".into());
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    let options_json = data["challenge"].to_string();

    // Step 2: Try conditional mediation (silent)
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

            // Use conditional mediation for silent auth (if supported)
            const requestOptions = {{ publicKey: options }};
            if (typeof PublicKeyCredential !== 'undefined' &&
                PublicKeyCredential.isConditionalMediationAvailable &&
                await PublicKeyCredential.isConditionalMediationAvailable()) {{
                requestOptions.mediation = 'conditional';
            }}

            const credential = await navigator.credentials.get(requestOptions);

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

    let eval_fn = Function::new_no_args(&format!("return {js_code}"));
    let promise = eval_fn.call0(&JsValue::NULL).map_err(|e| format!("JS error: {e:?}"))?;
    let result = JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("Silent auth failed: {e:?}"))?;

    let credential_json = result.as_string().ok_or("No result")?;

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
        Err("Silent auth failed".into())
    }
}
