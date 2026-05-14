use std::sync::Arc;

use leptos::prelude::*;

use crate::components;
use crate::pages;
use crate::pages::rate_beer::ActiveSession;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Page {
    Home,
    Login,
    Register,
    BeerList,
    BeerDetail(String),
    AddBeer,
    Profile,
    RateBeer,
    Sessions,
    MyTastings,
}

impl Page {
    fn is_home(&self) -> bool {
        matches!(self, Self::Home)
    }

    fn is_beers(&self) -> bool {
        matches!(self, Self::BeerList | Self::BeerDetail(_) | Self::AddBeer)
    }

    fn is_rate(&self) -> bool {
        matches!(self, Self::RateBeer)
    }

    fn is_sessions(&self) -> bool {
        matches!(self, Self::Sessions)
    }

    fn is_account(&self) -> bool {
        matches!(
            self,
            Self::Profile | Self::Login | Self::Register | Self::MyTastings
        )
    }
}

#[derive(Clone)]
pub struct HomePageNav {
    pub is_logged_in: Signal<bool>,
    pub login: Arc<dyn Fn() + Send + Sync>,
    pub register: Arc<dyn Fn() + Send + Sync>,
}

#[component]
pub fn App() -> impl IntoView {
    let (current_page, set_page) = signal(Page::Home);
    let (token, set_token) = signal(Option::<String>::None);
    let (active_session, set_active_session) = signal(Option::<ActiveSession>::None);

    provide_context(HomePageNav {
        is_logged_in: Signal::derive(move || token.get().is_some()),
        login: Arc::new({
            let set_page = set_page;
            move || set_page.set(Page::Login)
        }),
        register: Arc::new({
            let set_page = set_page;
            move || set_page.set(Page::Register)
        }),
    });

    // Auto-login: if autologon flag is set and we have a username, try passkey login
    {
        let set_token = set_token;
        let set_page = set_page;
        leptos::task::spawn_local(async move {
            use crate::pages::login::{
                do_passkey_login, extract_username_from_jwt, get_autologon, get_stored_username,
                set_autologon, store_username,
            };

            if !get_autologon() || get_stored_username().is_none() {
                return;
            }

            // Flip to false before attempting (prevents infinite retry on failure)
            set_autologon(false);

            match do_passkey_login().await {
                Ok(tok) => {
                    if let Some(name) = extract_username_from_jwt(&tok) {
                        store_username(&name);
                    }
                    set_autologon(true);
                    set_token.set(Some(tok));
                    set_page.set(Page::Home);
                }
                Err(_) => {
                    // Auto-login failed silently — user can log in manually
                }
            }
        });
    }

    let nav_class = "nav-link";

    let on_navigate = {
        let set_page = set_page;
        move |target: &str| match target {
            "login" => set_page.set(Page::Login),
            "register" => set_page.set(Page::Register),
            "profile" => set_page.set(Page::Profile),
            "add-beer" => set_page.set(Page::AddBeer),
            "rate-beer" => set_page.set(Page::RateBeer),
            "my-tastings" => set_page.set(Page::MyTastings),
            _ => {}
        }
    };

    view! {
        <div class="app">
            <header class="header">
                <button
                    type="button"
                    class="logo"
                    aria-label="Go to home feed"
                    on:click=move |_| set_page.set(Page::Home)
                >
                    "🍺 Open Tappd"
                </button>

                <nav class="nav" aria-label="Primary navigation">
                    <button
                        type="button"
                        class=nav_class
                        class:active=move || current_page.with(|page| page.is_home())
                        on:click=move |_| set_page.set(Page::Home)
                    >
                        "Feed"
                    </button>
                    <button
                        type="button"
                        class=nav_class
                        class:active=move || current_page.with(|page| page.is_beers())
                        on:click=move |_| set_page.set(Page::BeerList)
                    >
                        "Beers"
                    </button>
                    <button
                        type="button"
                        class=nav_class
                        class:active=move || current_page.with(|page| page.is_rate())
                        on:click=move |_| set_page.set(Page::RateBeer)
                    >
                        "Rate"
                    </button>
                    <button
                        type="button"
                        class=nav_class
                        class:active=move || current_page.with(|page| page.is_sessions())
                        on:click=move |_| set_page.set(Page::Sessions)
                    >
                        "Sessions"
                    </button>
                    {move || active_session.get().map(|session| view! {
                        <span class="active-session-indicator" title=format!("Active: {}", session.name)>
                            "📋"
                        </span>
                    })}
                    <components::user_menu::UserMenu
                        token=token
                        set_token=set_token
                        on_navigate=on_navigate
                    />
                </nav>
            </header>

            <main class="content">
                {move || {
                    let page = current_page.get();
                    match page {
                        Page::Home => view! { <pages::home::HomePage /> }.into_any(),
                        Page::Login => view! {
                            <pages::login::LoginPage
                                token=set_token
                                on_success=move || set_page.set(Page::Home)
                            />
                        }
                        .into_any(),
                        Page::Register => view! {
                            <pages::register::RegisterPage
                                token=set_token
                                on_success=move || set_page.set(Page::Home)
                            />
                        }
                        .into_any(),
                        Page::BeerList => view! {
                            <pages::beer_list::BeerListPage
                                token=token
                                on_view_beer=move |id: String| set_page.set(Page::BeerDetail(id))
                                on_add_beer=move || set_page.set(Page::AddBeer)
                            />
                        }
                        .into_any(),
                        Page::BeerDetail(id) => view! {
                            <pages::beer_detail::BeerDetailPage
                                beer_id=id
                                token=token
                                on_back=move || set_page.set(Page::BeerList)
                            />
                        }
                        .into_any(),
                        Page::AddBeer => view! {
                            <pages::add_beer::AddBeerPage token=token />
                        }
                        .into_any(),
                        Page::Profile => view! { <pages::profile::ProfilePage token=token /> }.into_any(),
                        Page::RateBeer => view! {
                            <pages::rate_beer::RateBeerPage
                                token=token
                                active_session=active_session
                                set_active_session=set_active_session
                            />
                        }
                        .into_any(),
                        Page::Sessions => view! {
                            <pages::sessions::SessionBrowserPage
                                token=token
                                active_session=active_session
                                set_active_session=set_active_session
                            />
                        }
                        .into_any(),
                        Page::MyTastings => view! {
                            <pages::my_tastings::MyTastingsPage token=token />
                        }
                        .into_any(),
                    }
                }}
            </main>

            <Show when=move || current_page.with(|page| page.is_home())>
                <button
                    type="button"
                    class="fab"
                    aria-label="Add tasting"
                    on:click=move |_| set_page.set(Page::RateBeer)
                >
                    "+"
                </button>
            </Show>

            <nav class="bottom-nav" aria-label="Mobile navigation">
                <button
                    type="button"
                    class="bottom-nav-item"
                    class:active=move || current_page.with(|page| page.is_home())
                    on:click=move |_| set_page.set(Page::Home)
                >
                    <span class="bottom-nav-icon">"🏠"</span>
                    <span class="bottom-nav-label">"Home"</span>
                </button>
                <button
                    type="button"
                    class="bottom-nav-item"
                    class:active=move || current_page.with(|page| page.is_rate())
                    on:click=move |_| set_page.set(Page::RateBeer)
                >
                    <span class="bottom-nav-icon">"🍺"</span>
                    <span class="bottom-nav-label">"Rate"</span>
                </button>
                <button
                    type="button"
                    class="bottom-nav-item"
                    class:active=move || current_page.with(|page| page.is_sessions())
                    on:click=move |_| set_page.set(Page::Sessions)
                >
                    <span class="bottom-nav-icon">"📋"</span>
                    <span class="bottom-nav-label">"Sessions"</span>
                </button>
                <button
                    type="button"
                    class="bottom-nav-item"
                    class:active=move || current_page.with(|page| page.is_account())
                    on:click=move |_| {
                        if token.get().is_some() {
                            set_page.set(Page::Profile);
                        } else {
                            set_page.set(Page::Login);
                        }
                    }
                >
                    <span class="bottom-nav-icon">"👤"</span>
                    <span class="bottom-nav-label">
                        {move || if token.get().is_some() { "Profile" } else { "Login" }}
                    </span>
                </button>
            </nav>

            <footer class="footer">
                <p>"Open Tappd — Community-owned, privacy-first beer tasting 🍺"</p>
                <p class="privacy-note">"Your data is yours. Private by default."</p>
            </footer>
        </div>
    }
}
