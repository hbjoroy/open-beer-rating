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

#[component]
pub fn App() -> impl IntoView {
    let (current_page, set_page) = signal(Page::Home);
    let (token, set_token) = signal(Option::<String>::None);
    let (active_session, set_active_session) = signal(Option::<ActiveSession>::None);

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
                <h1 class="logo" on:click=move |_| set_page.set(Page::Home) style="cursor: pointer;">"🍺 Open Tappd"</h1>
                <nav class="nav">
                    <a class=nav_class on:click=move |_| set_page.set(Page::BeerList)>"Beers"</a>
                    <a class=nav_class on:click=move |_| set_page.set(Page::RateBeer)>"Rate"</a>
                    <a class=nav_class on:click=move |_| set_page.set(Page::Sessions)>"Sessions"</a>
                    {move || active_session.get().map(|s| view! {
                        <span class="active-session-indicator" title=format!("Active: {}", s.name)>
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
                        Page::RateBeer => view! {
                            <pages::rate_beer::RateBeerPage
                                token=token
                                active_session=active_session
                            />
                        }.into_any(),
                        Page::Sessions => view! {
                            <pages::sessions::SessionBrowserPage
                                token=token
                                active_session=active_session
                                set_active_session=set_active_session
                            />
                        }.into_any(),
                        Page::MyTastings => view! {
                            <pages::my_tastings::MyTastingsPage token=token />
                        }.into_any(),
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
