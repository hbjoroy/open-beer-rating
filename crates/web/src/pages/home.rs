use leptos::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct RecentTasting {
    id: String,
    username: String,
    beer_id: String,
    beer_name: String,
    beer_style: Option<String>,
    brewery_name: String,
    score: i32,
    serving_style: Option<String>,
    location_name: Option<String>,
    session_name: Option<String>,
    tasted_at: String,
}

#[component]
pub fn HomePage() -> impl IntoView {
    let (tastings, set_tastings) = signal(Vec::<RecentTasting>::new());
    let (loading, set_loading) = signal(true);

    // Load recent tastings on mount
    leptos::task::spawn_local(async move {
        match fetch_recent_tastings().await {
            Ok(list) => set_tastings.set(list),
            Err(_) => {} // silently degrade to empty feed
        }
        set_loading.set(false);
    });

    view! {
        <div class="page home">
            <section class="hero">
                <h2>"🍺 Open Tappd"</h2>
                <p class="tagline">"Community-owned, privacy-first beer tasting"</p>
            </section>

            <section class="recent-feed">
                <h3>"Recent Check-ins"</h3>
                {move || {
                    if loading.get() {
                        return view! { <p class="loading">"Loading..."</p> }.into_any();
                    }
                    let list = tastings.get();
                    if list.is_empty() {
                        return view! {
                            <p class="empty">"No check-ins yet. Be the first to rate a beer!"</p>
                        }.into_any();
                    }
                    view! {
                        <div class="feed-list">
                            {list.iter().map(|t| {
                                let score_class = match t.score {
                                    0..=2 => "poor",
                                    3..=4 => "below-avg",
                                    5 => "average",
                                    6..=7 => "good",
                                    8..=9 => "excellent",
                                    10 => "world-class",
                                    _ => "",
                                };
                                let serving = t.serving_style.as_ref()
                                    .map(|s| format_serving_style(s))
                                    .unwrap_or_default();
                                view! {
                                    <div class="feed-item">
                                        <div class="feed-item-header">
                                            <span class="feed-username">{t.username.clone()}</span>
                                            <span class="feed-action">" is drinking"</span>
                                        </div>
                                        <div class="feed-item-body">
                                            <div class="feed-beer-info">
                                                <strong class="feed-beer-name">{t.beer_name.clone()}</strong>
                                                <span class="feed-brewery">{t.brewery_name.clone()}</span>
                                                {t.beer_style.as_ref().map(|s| view! {
                                                    <span class="feed-style">{s.clone()}</span>
                                                })}
                                            </div>
                                            <span class=format!("score-badge {score_class}")>
                                                {format!("{}/10", t.score)}
                                            </span>
                                        </div>
                                        <div class="feed-item-meta">
                                            <span class="feed-date">{format_date(&t.tasted_at)}</span>
                                            {(!serving.is_empty()).then(|| view! {
                                                <span class="feed-serving">{serving.clone()}</span>
                                            })}
                                            {t.location_name.as_ref().map(|l| view! {
                                                <span class="feed-location">"📍 " {l.clone()}</span>
                                            })}
                                            {t.session_name.as_ref().map(|s| view! {
                                                <span class="feed-session">"📋 " {s.clone()}</span>
                                            })}
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                }}
            </section>
        </div>
    }
}

fn format_date(iso: &str) -> String {
    if iso.len() >= 16 {
        // YYYY-MM-DDTHH:MM
        format!("{} {}", &iso[..10], &iso[11..16])
    } else if iso.len() >= 10 {
        iso[..10].to_string()
    } else {
        iso.to_string()
    }
}

fn format_serving_style(style: &str) -> String {
    match style {
        "draft" => "🍺 Draft".to_string(),
        "bottle" => "🍾 Bottle".to_string(),
        "can" => "🥫 Can".to_string(),
        "cask" => "🪵 Cask".to_string(),
        "crowler" => "Crowler".to_string(),
        "growler" => "Growler".to_string(),
        "nitro" => "☁️ Nitro".to_string(),
        "taster" => "Taster".to_string(),
        _ => style.to_string(),
    }
}

async fn fetch_recent_tastings() -> Result<Vec<RecentTasting>, String> {
    let resp = gloo_net::http::Request::get("/api/tastings/recent?limit=20")
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<RecentTasting>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to fetch recent tastings".to_string())
    }
}
