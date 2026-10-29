use crate::pages::rate_beer::ActiveSession;
use crate::{components, pages};
use leptos::prelude::*;
use leptos_router::{
    components::{A, Route, Router, Routes},
    hooks::{use_location, use_navigate, use_params_map, use_query_map},
    path,
};
use serde::Deserialize;

#[derive(Clone)]
pub struct HomePageNav {
    pub is_logged_in: Signal<bool>,
}

#[component]
pub fn App() -> impl IntoView {
    view! { <Router><AppShell /></Router> }
}

#[component]
fn AppShell() -> impl IntoView {
    let (token, set_token) = signal(Option::<String>::None);
    let (active_session, set_active_session) = signal(Option::<ActiveSession>::None);
    let ready_owner = RwSignal::new(Option::<String>::None);
    let previous_owner = StoredValue::new(Option::<String>::None);
    let navigate = use_navigate();
    let go = Callback::new(move |path: String| navigate(&path, Default::default()));
    let query = use_query_map();
    let location = use_location();
    let auth_success =
        Callback::new(move |()| go.run(safe_next(query.read().get("next").as_deref()).to_string()));
    provide_context(HomePageNav {
        is_logged_in: Signal::derive(move || token.get().is_some()),
    });
    // Old selections were shared across accounts and cannot be trusted.
    pages::rate_beer::clear_persisted_session();

    Effect::new(move |_| {
        let current_token = token.get();
        let owner = current_token.as_deref().and_then(account_id);
        let old_owner = previous_owner.get_value();
        if old_owner != owner {
            if let Some(old) = old_owner {
                remove_session(&old);
                pages::rate_beer::clear_tasting_drafts();
            }
            previous_owner.set_value(owner.clone());
        }
        ready_owner.set(None);
        set_active_session.set(None);
        let (Some(owner), Some(tok)) = (owner, current_token) else {
            return;
        };
        let saved = saved_session(&owner);
        leptos::task::spawn_local(async move {
            let restored = match saved {
                Some(id) => validate_session(&tok, &id).await,
                None => None,
            };
            // An old response must not select a session after logout/account change.
            if token.get_untracked().as_deref() != Some(tok.as_str()) {
                return;
            }
            set_active_session.set(restored);
            ready_owner.set(Some(owner));
        });
    });
    Effect::new(move |_| {
        let selected = active_session.get();
        if let Some(owner) = ready_owner.get() {
            if token.get().as_deref().and_then(account_id).as_ref() != Some(&owner) {
                return;
            }
            if let Some(storage) = local_storage() {
                let key = session_key(&owner);
                match selected {
                    Some(session) => {
                        let _ = storage.set_item(&key, &session.id);
                    }
                    None => {
                        let _ = storage.remove_item(&key);
                    }
                }
            }
        }
    });
    leptos::task::spawn_local(async move {
        use pages::login::{
            do_passkey_login, extract_username_from_jwt, get_autologon, get_stored_username,
            set_autologon, store_username,
        };
        if matches!(location.pathname.get_untracked().as_str(), "/identity/finish" | "/email-recovery") || !get_autologon() || get_stored_username().is_none() {
            return;
        }
        set_autologon(false);
        if let Ok(tok) = do_passkey_login().await {
            if let Some(name) = extract_username_from_jwt(&tok) {
                store_username(&name);
            }
            set_autologon(true);
            set_token.set(Some(tok));
            if matches!(
                location.pathname.get_untracked().as_str(),
                "/login" | "/register"
            ) {
                auth_success.run(());
            }
        }
    });
    let on_navigate = move |target: &str| {
        if matches!(target, "login" | "register") {
            let pathname = location.pathname.get_untracked();
            let search = location.search.get_untracked();
            let destination = if matches!(pathname.as_str(), "/login" | "/register") {
                safe_next(query.get_untracked().get("next").as_deref()).to_string()
            } else if search.is_empty() {
                pathname
            } else {
                format!("{pathname}?{search}")
            };
            let next = js_sys::encode_uri_component(safe_next(Some(&destination)))
                .as_string()
                .unwrap_or_else(|| "/".to_string());
            go.run(format!("/{target}?next={next}"));
            return;
        }
        let path = match target {
            "profile" => "/account",
            "add-beer" => "/beers/new",
            "rate-beer" => "/tastings/new",
            "my-tastings" => "/tastings",
            _ => return,
        };
        go.run(path.to_string());
    };
    view! {
        <div class="app">
            <a class="skip-link" href="#main-content">"Skip to main content"</a>
            <header class="header">
                <A href="/" attr:class="logo" exact=true>"Open Tappd"</A>
                <nav class="nav" aria-label="Primary navigation">
                    <A href="/" exact=true attr:class="nav-link">"Home"</A>
                    <A href="/beers" attr:class="nav-link">"Beers"</A>
                    <A href="/tastings/new" exact=true attr:class="nav-link">"Add tasting"</A>
                    <A href="/sessions" attr:class="nav-link">"Sessions"</A>
                    <A href="/tastings" exact=true attr:class="nav-link">"My tastings"</A>
                </nav>
                <div class="header-account"><components::user_menu::UserMenu token=token set_token=set_token on_navigate=on_navigate /></div>
            </header>
            {move || active_session.get().map(|session| view! {
                <aside class="global-session-banner" aria-label="Selected tasting session">
                    <span>"New tastings use " <strong>{session.name}</strong></span>
                    <button type="button" class="btn-text" on:click=move |_| set_active_session.set(None)>"Use no session"</button>
                </aside>
            })}
            <main id="main-content" class="content" tabindex="-1">
                <Routes fallback=|| view! {
                    <div class="page"><h1>"Page not found"</h1><p>"This address does not match a page."</p><A href="/">"Go home"</A></div>
                }>
                    <Route path=path!("/") view=pages::home::HomePage />
                    <Route path=path!("/login") view=move || view! {
                        <pages::login::LoginPage token=set_token on_success=move || auth_success.run(()) />
                    } />
                    <Route path=path!("/register") view=move || view! {
                        <pages::register::RegisterPage token=set_token on_success=move || auth_success.run(()) />
                    } />
                    <Route path=path!("/beers") view=move || view! {
                        <pages::beer_list::BeerListPage token=token on_view_beer=move |id: String| go.run(format!("/beers/{id}")) on_add_beer=move || go.run("/beers/new".to_string()) />
                    } />
                    <Route path=path!("/beers/new") view=move || view! { <pages::add_beer::AddBeerPage token=token /> } />
                    <Route path=path!("/beers/:id") view=move || {
                        let params = use_params_map();
                        move || {
                            let id = params.read().get("id").unwrap_or_default();
                            view! { <pages::beer_detail::BeerDetailPage beer_id=id token=token on_back=move || go.run("/beers".to_string()) /> }
                        }
                    } />
                    <Route path=path!("/tastings/new") view=move || view! {
                        <pages::rate_beer::RateBeerPage token=token active_session=active_session set_active_session=set_active_session />
                    } />
                    <Route path=path!("/tastings") view=move || view! { <pages::my_tastings::MyTastingsPage token=token /> } />
                    <Route path=path!("/sessions") view=move || view! {
                        <pages::sessions::SessionBrowserPage token=token active_session=active_session set_active_session=set_active_session />
                    } />
                    <Route path=path!("/identity/finish") view=move || view! { <pages::identity::IdentityFinish token=set_token /> } />
                    <Route path=path!("/email-recovery") view=move || view! { <pages::email_recovery::EmailRecoveryPage token=set_token /> } />
                    <Route path=path!("/account") view=move || view! { <pages::profile::ProfilePage token=token /> } />
                </Routes>
            </main>
            <nav class="bottom-nav" aria-label="Mobile navigation">
                <A href="/" exact=true attr:class="bottom-nav-item"><span class="bottom-nav-icon" aria-hidden="true">"⌂"</span><span class="bottom-nav-label">"Home"</span></A>
                <A href="/beers" attr:class="bottom-nav-item"><span class="bottom-nav-icon" aria-hidden="true">"≡"</span><span class="bottom-nav-label">"Beers"</span></A>
                <A href="/tastings/new" exact=true attr:class="bottom-nav-item"><span class="bottom-nav-icon" aria-hidden="true">"+"</span><span class="bottom-nav-label">"Add tasting"</span></A>
                <A href="/sessions" attr:class="bottom-nav-item"><span class="bottom-nav-icon" aria-hidden="true">"◎"</span><span class="bottom-nav-label">"Sessions"</span></A>
                <A href="/tastings" exact=true attr:class="bottom-nav-item"><span class="bottom-nav-icon" aria-hidden="true">"▤"</span><span class="bottom-nav-label">"My tastings"</span></A>
            </nav>
            <footer class="footer"><p>"Open Tappd — your private beer journal"</p><p class="privacy-note">"Your data is yours. Private by default."</p></footer>
        </div>
    }
}

/// Accept local destinations only, including query strings.
fn safe_next(next: Option<&str>) -> &str {
    next.filter(|path| {
        path.starts_with('/')
            && !path.starts_with("//")
            && !path.contains('\\')
            && !path.chars().any(char::is_control)
            && !path.to_ascii_lowercase().contains("%5c")
            && !path.to_ascii_lowercase().contains("%2f")
            && !matches!(path.split(['?', '#']).next(), Some("/login" | "/register"))
    })
    .unwrap_or("/")
}
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
fn session_key(owner: &str) -> String {
    format!("open_tappd_active_session:{owner}")
}
fn saved_session(owner: &str) -> Option<String> {
    local_storage()?
        .get_item(&session_key(owner))
        .ok()
        .flatten()
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
}
fn remove_session(owner: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(&session_key(owner));
    }
}
// Partitions preferences only; API authorization validates the JWT.
fn account_id(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?.replace('-', "+").replace('_', "/");
    let padded = format!("{}{}", payload, "=".repeat((4 - payload.len() % 4) % 4));
    let decoded = web_sys::window()?.atob(&padded).ok()?;
    let bytes: Vec<u8> = decoded.chars().map(|c| c as u8).collect();
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    uuid::Uuid::parse_str(claims.get("sub")?.as_str()?)
        .ok()
        .map(|id| id.to_string())
}
#[derive(Deserialize)]
struct RememberedSession {
    id: String,
    name: String,
    ended_at: Option<String>,
}
async fn validate_session(token: &str, id: &str) -> Option<ActiveSession> {
    let response = gloo_net::http::Request::get(&format!("/api/tasting-sessions/{id}"))
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .ok()?;
    if !response.ok() {
        return None;
    }
    let session: RememberedSession = response.json().await.ok()?;
    // This endpoint authorizes the creator or a current participant, including
    // private sessions whose participant lists are intentionally hidden.
    if session.ended_at.is_some() {
        return None;
    }
    Some(ActiveSession {
        id: session.id,
        name: session.name,
    })
}
#[cfg(test)]
mod tests {
    use super::safe_next;
    #[test]
    fn accepts_local_return_paths() {
        assert_eq!(
            safe_next(Some("/tastings/new?beer=123")),
            "/tastings/new?beer=123"
        );
        assert_eq!(safe_next(Some("/beers")), "/beers");
    }
    #[test]
    fn rejects_external_and_ambiguous_return_paths() {
        for value in [
            None,
            Some("https://evil.example"),
            Some("//evil.example"),
            Some("/\\evil.example"),
            Some("/%2f%2fevil.example"),
            Some("/%5cevil.example"),
            Some("/\nevil.example"),
            Some("/login?next=/login"),
            Some("/register"),
        ] {
            assert_eq!(safe_next(value), "/");
        }
    }
}
