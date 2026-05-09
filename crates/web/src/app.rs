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
                        Page::Register => view! { <pages::register::RegisterPage on_success=move || set_page.set(Page::Login) /> }.into_any(),
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
