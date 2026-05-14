use leptos::prelude::*;

/// Decoded JWT claims (client-side, no signature verification)
#[derive(Debug, Clone, serde::Deserialize)]
struct JwtPayload {
    username: String,
}

fn decode_username(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let padded = match parts[1].len() % 4 {
        2 => format!("{}==", parts[1]),
        3 => format!("{}=", parts[1]),
        _ => parts[1].to_string(),
    };
    let b64 = padded.replace('-', "+").replace('_', "/");
    let window = web_sys::window()?;
    let decoded = window.atob(&b64).ok()?;
    let bytes: Vec<u8> = decoded.chars().map(|c| c as u8).collect();
    let payload: JwtPayload = serde_json::from_slice(&bytes).ok()?;
    Some(payload.username)
}

#[component]
pub fn UserMenu(
    token: ReadSignal<Option<String>>,
    set_token: WriteSignal<Option<String>>,
    on_navigate: impl Fn(&str) + Send + Sync + Clone + 'static,
) -> impl IntoView {
    let (menu_open, set_menu_open) = signal(false);

    let username = Memo::new(move |_| token.get().and_then(|t| decode_username(&t)));

    let toggle_menu = move |_| {
        set_menu_open.update(|open| *open = !*open);
    };

    let close_menu = move || {
        set_menu_open.set(false);
    };

    view! {
        <div class="user-menu">
            {move || {
                let on_nav = on_navigate.clone();
                if token.get().is_some() {
                    let display = username.get().unwrap_or_else(|| "User".to_string());
                    let on_nav_profile = on_nav.clone();
                    let on_nav_add = on_nav.clone();
                    let on_nav_tastings = on_nav.clone();
                    view! {
                        <button class="user-menu-trigger logged-in" on:click=toggle_menu>
                            <span class="user-avatar">"👤"</span>
                            <span class="user-name">{display}</span>
                            <span class="dropdown-arrow">{move || if menu_open.get() { "▲" } else { "▼" }}</span>
                        </button>
                        <div class="user-dropdown" class:open=move || menu_open.get()>
                            <a class="dropdown-item" on:click=move |_| { close_menu(); on_nav_add("rate-beer"); }>"🍺 Rate a Beer"</a>
                            <a class="dropdown-item" on:click=move |_| { close_menu(); on_nav_tastings("my-tastings"); }>"📝 My Tastings"</a>
                            <div class="dropdown-divider"></div>
                            <a class="dropdown-item" on:click=move |_| { close_menu(); on_nav_profile("profile"); }>"⚙️ Profile"</a>
                            <a class="dropdown-item" on:click=move |_| { close_menu(); crate::pages::login::set_autologon(false); set_token.set(None); }>"🚪 Sign Out"</a>
                        </div>
                    }.into_any()
                } else {
                    let on_nav_login = on_nav.clone();
                    let on_nav_register = on_nav.clone();
                    view! {
                        <button class="user-menu-trigger logged-out" on:click=toggle_menu>
                            <span class="user-avatar">"👤"</span>
                            <span class="user-name">"Sign In"</span>
                            <span class="dropdown-arrow">{move || if menu_open.get() { "▲" } else { "▼" }}</span>
                        </button>
                        <div class="user-dropdown" class:open=move || menu_open.get()>
                            <a class="dropdown-item" on:click=move |_| { close_menu(); on_nav_login("login"); }>"🔑 Sign In"</a>
                            <a class="dropdown-item" on:click=move |_| { close_menu(); on_nav_register("register"); }>"📝 Create Account"</a>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
