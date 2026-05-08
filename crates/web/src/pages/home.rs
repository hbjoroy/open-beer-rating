use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="page home">
            <section class="hero">
                <h2>"Welcome to Open Tappd"</h2>
                <p class="tagline">"A community-owned, privacy-first beer tasting platform"</p>
                <p>"Rate beers on a 0–10 scale. Earn badges. Own your data."</p>
            </section>

            <section class="features">
                <div class="feature">
                    <h3>"🔒 Privacy First"</h3>
                    <p>"Your drinking history is private by default. No tracking, no analytics. Your data, your rules."</p>
                </div>
                <div class="feature">
                    <h3>"🏆 Gamification"</h3>
                    <p>"Earn badges as you explore: First Sip, Explorer, Connoisseur, Style Hunter, and more."</p>
                </div>
                <div class="feature">
                    <h3>"🌍 Open Source"</h3>
                    <p>"MIT licensed. Community-owned. No corporate overlords."</p>
                </div>
                <div class="feature">
                    <h3>"📊 0–10 Scale"</h3>
                    <p>"More granularity than 0–5. Rate with precision."</p>
                </div>
            </section>
        </div>
    }
}
