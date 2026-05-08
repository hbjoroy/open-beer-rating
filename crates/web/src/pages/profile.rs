use leptos::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct BadgeResponse {
    name: String,
    description: String,
}

#[derive(Debug, Clone, Deserialize)]
struct UserRating {
    beer_name: String,
    brewery_name: String,
    score: i32,
}

#[component]
pub fn ProfilePage(token: ReadSignal<Option<String>>) -> impl IntoView {
    let badges = leptos::prelude::LocalResource::new(move || {
        let tok = token.get();
        async move {
            match tok {
                Some(t) => fetch_badges(&t).await,
                None => Err("Not logged in".to_string()),
            }
        }
    });

    let ratings = leptos::prelude::LocalResource::new(move || {
        let tok = token.get();
        async move {
            match tok {
                Some(t) => fetch_my_ratings(&t).await,
                None => Err("Not logged in".to_string()),
            }
        }
    });

    view! {
        <div class="page profile">
            <h2>"My Profile"</h2>

            <section>
                <h3>"🏆 Badges"</h3>
                <Suspense fallback=move || view! { <p>"Loading badges..."</p> }>
                    {move || {
                        badges.get().map(|result| {
                            match &*result {
                                Ok(badge_list) => {
                                    if badge_list.is_empty() {
                                        view! { <p>"No badges yet. Start rating beers!"</p> }.into_any()
                                    } else {
                                        view! {
                                            <div class="badge-list">
                                                {badge_list.iter().map(|b| {
                                                    view! {
                                                        <div class="badge">
                                                            <strong>{b.name.clone()}</strong>
                                                            <span>{b.description.clone()}</span>
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }
                                }
                                Err(e) => view! { <p class="error">{format!("Error: {e}")}</p> }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </section>

            <section>
                <h3>"📝 My Ratings"</h3>
                <Suspense fallback=move || view! { <p>"Loading ratings..."</p> }>
                    {move || {
                        ratings.get().map(|result| {
                            match &*result {
                                Ok(rating_list) => {
                                    if rating_list.is_empty() {
                                        view! { <p>"No ratings yet."</p> }.into_any()
                                    } else {
                                        view! {
                                            <div class="rating-list">
                                                {rating_list.iter().map(|r| {
                                                    view! {
                                                        <div class="rating-card">
                                                            <strong>{r.beer_name.clone()}</strong>
                                                            <span class="brewery">{r.brewery_name.clone()}</span>
                                                            <span class="score">{format!("{}/10", r.score)}</span>
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }
                                }
                                Err(e) => view! { <p class="error">{format!("Error: {e}")}</p> }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </section>
        </div>
    }
}

async fn fetch_badges(token: &str) -> Result<Vec<BadgeResponse>, String> {
    let resp = gloo_net::http::Request::get("/api/users/me/badges")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<BadgeResponse>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to fetch badges".to_string())
    }
}

async fn fetch_my_ratings(token: &str) -> Result<Vec<UserRating>, String> {
    let resp = gloo_net::http::Request::get("/api/users/me/ratings")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<UserRating>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to fetch ratings".to_string())
    }
}
