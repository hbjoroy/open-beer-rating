use crate::app::HomePageNav;
use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn HomePage() -> impl IntoView {
    let nav = use_context::<HomePageNav>().expect("App provides home navigation");
    view! {
        <div class="page home">
            <section class="hero">
                <div class="hero-copy">
                    <span class="hero-kicker">"Your beer journal"</span>
                    <h1>"Make the next sip memorable."</h1>
                    <p class="tagline">"Record a tasting, find a beer, or look back at your own notes. Your tastings are private by default."</p>
                    <div class="welcome-actions">
                        <A href="/tastings/new" attr:class="btn-primary">"Add a tasting"</A>
                        <A href="/beers" attr:class="btn-secondary">"Explore beers"</A>
                    </div>
                </div>
                <Show when=move || !nav.is_logged_in.get()>
                    <div class="welcome-banner">
                        <div class="welcome-copy"><strong>"Keep a journal of your own"</strong><p>"Sign in to save private tastings and join a shared tasting session."</p></div>
                        <div class="welcome-actions">
                            <A href="/login?next=/tastings/new" attr:class="btn-primary">"Sign in"</A>
                            <A href="/register?next=/tastings/new" attr:class="btn-secondary">"Create account"</A>
                        </div>
                    </div>
                </Show>
            </section>
            <section aria-labelledby="home-actions-heading">
                <div class="section-heading"><h2 id="home-actions-heading">"What would you like to do?"</h2></div>
                <div class="task-grid">
                    <A href="/tastings/new" attr:class="task-card"><h3>"Record a tasting"</h3><p>"Choose a beer, score it, and keep the details you want to remember."</p><span>"Add tasting →"</span></A>
                    <A href="/beers" attr:class="task-card"><h3>"Find a beer"</h3><p>"Search the catalogue by beer name or brewery."</p><span>"Browse beers →"</span></A>
                    <A href="/tastings" attr:class="task-card"><h3>"Your tasting history"</h3><p>"Revisit your ratings and private notes."</p><span>"My tastings →"</span></A>
                    <A href="/sessions" attr:class="task-card"><h3>"Taste together"</h3><p>"Create or join a session and choose which tastings belong to it."</p><span>"Open sessions →"</span></A>
                </div>
            </section>
        </div>
    }
}
