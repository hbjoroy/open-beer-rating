use leptos::prelude::*;
use serde::Deserialize;

use crate::components::score_badge::ScoreBadge;

#[derive(Debug, Clone, Deserialize)]
struct BeerDetail {
    brewery_id: String,
    name: String,
    style: Option<String>,
    abv: Option<f64>,
    description: Option<String>,
    average_score: Option<f64>,
    rating_count: i64,
    total_tastings: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct BreweryInfo {
    name: String,
    country: Option<String>,
    city: Option<String>,
}

#[component]
pub fn BeerDetailPage(
    beer_id: String,
    token: ReadSignal<Option<String>>,
    on_back: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let (refresh, set_refresh) = signal(0u32);
    let (selected_score, set_selected_score) = signal(Option::<i32>::None);
    let (notes, set_notes) = signal(String::new());
    let (rating_error, set_rating_error) = signal(Option::<String>::None);
    let (rating_success, set_rating_success) = signal(Option::<String>::None);
    let (submitting, set_submitting) = signal(false);
    let on_back = std::sync::Arc::new(on_back);

    let beer_id_res = beer_id.clone();
    let detail = leptos::prelude::LocalResource::new(move || {
        refresh.get();
        let id = beer_id_res.clone();
        async move { fetch_beer_with_brewery(&id).await }
    });

    let beer_id_rate = beer_id.clone();
    let on_rate = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let score = match selected_score.get() {
            Some(s) => s,
            None => {
                set_rating_error.set(Some("Please select a score".to_string()));
                return;
            }
        };
        let tok = match token.get() {
            Some(t) => t,
            None => {
                set_rating_error.set(Some("Please log in to rate".to_string()));
                return;
            }
        };

        set_submitting.set(true);
        set_rating_error.set(None);
        set_rating_success.set(None);

        let bid = beer_id_rate.clone();
        let notes_val = notes.get();

        leptos::task::spawn_local(async move {
            match submit_rating(
                &tok,
                &bid,
                score,
                if notes_val.is_empty() {
                    None
                } else {
                    Some(&notes_val)
                },
            )
            .await
            {
                Ok(()) => {
                    set_rating_success.set(Some(format!("Rated {}/10! 🎉", score)));
                    set_selected_score.set(None);
                    set_notes.set(String::new());
                    set_refresh.update(|v| *v += 1);
                }
                Err(e) => set_rating_error.set(Some(e)),
            }
            set_submitting.set(false);
        });
    };

    let back_click = on_back.clone();

    view! {
        <div class="page beer-detail">
            <a class="back-link" on:click=move |_| back_click()>"← Back to beers"</a>

            <Suspense fallback=move || view! { <p>"Loading..."</p> }>
                {move || {
                    detail.get().map(|result| {
                        match &*result {
                            Ok((beer, brewery)) => {
                                let avg = beer.average_score;
                                let count = beer.rating_count;
                                view! {
                                    <div class="detail-card">
                                        <h2>{beer.name.clone()}</h2>
                                        <p class="brewery-info">
                                            "by " <strong>{brewery.name.clone()}</strong>
                                            {brewery.city.as_ref().map(|c| format!(", {c}"))}
                                            {brewery.country.as_ref().map(|c| format!(" ({})", c))}
                                        </p>
                                        <div class="beer-meta">
                                            {beer.style.as_ref().map(|s| view! { <span class="style">{s.clone()}</span> })}
                                            {beer.abv.map(|a| view! { <span class="abv">{format!("{:.1}% ABV", a)}</span> })}
                                        </div>
                                        {beer.description.as_ref().map(|d| view! { <p class="description">{d.clone()}</p> })}
                                        <div class="aggregate-score">
                                            {avg.map(|a| {
                                                let score_int = a.round() as i32;
                                                view! {
                                                    <div class="big-score-display">
                                                        <ScoreBadge score=score_int />
                                                        <span class="avg-label">{format!("{:.1} avg", a)}</span>
                                                    </div>
                                                }
                                            })}
                                            <span class="count">
                                                {format!("{} taster{}", count, if count == 1 { "" } else { "s" })}
                                                {beer.total_tastings.filter(|&t| t > count).map(|t| format!(" ({t} tastings)"))}
                                            </span>
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            Err(e) => view! { <p class="error">{format!("Error: {e}")}</p> }.into_any(),
                        }
                    })
                }}
            </Suspense>

            <div class="rate-section">
                <h3>"Rate this beer"</h3>

                {move || {
                    if token.get().is_none() {
                        Some(view! { <p class="login-prompt">"Log in to rate this beer"</p> })
                    } else {
                        None
                    }
                }}

                {move || rating_success.get().map(|msg| view! { <p class="success">{msg}</p> })}
                {move || rating_error.get().map(|e| view! { <p class="error">{e}</p> })}

                <form on:submit=on_rate style:display=move || if token.get().is_some() { "block" } else { "none" }>
                    <label class="form-label">"Your score"</label>
                    <div class="score-selector">
                        {(0..=10i32).map(|n| {
                            view! {
                                <button
                                    type="button"
                                    class=move || {
                                        let base = match n {
                                            0..=2 => "score-btn poor",
                                            3..=4 => "score-btn below-avg",
                                            5 => "score-btn average",
                                            6..=7 => "score-btn good",
                                            8..=9 => "score-btn excellent",
                                            10 => "score-btn world-class",
                                            _ => "score-btn",
                                        };
                                        if selected_score.get() == Some(n) {
                                            format!("{base} selected")
                                        } else {
                                            base.to_string()
                                        }
                                    }
                                    on:click=move |_| set_selected_score.set(Some(n))
                                >
                                    {n}
                                </button>
                            }
                        }).collect::<Vec<_>>()}
                    </div>

                    <div class="form-group">
                        <label>"Notes (optional, encrypted)"</label>
                        <textarea
                            prop:value=move || notes.get()
                            on:input=move |ev| set_notes.set(event_target_value(&ev))
                            rows="2"
                            placeholder="Your private tasting notes..."
                        ></textarea>
                    </div>

                    <button type="submit" disabled=move || submitting.get()>
                        {move || if submitting.get() { "Submitting..." } else { "Submit Tasting" }}
                    </button>
                </form>
            </div>
        </div>
    }
}

async fn fetch_beer_with_brewery(id: &str) -> Result<(BeerDetail, BreweryInfo), String> {
    let resp = gloo_net::http::Request::get(&format!("/api/beers/{id}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !resp.ok() {
        return Err("Beer not found".to_string());
    }

    let mut beer: BeerDetail = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {e}"))?;

    // Fetch tastings aggregate
    if let Ok(agg_resp) = gloo_net::http::Request::get(&format!("/api/beers/{id}/tastings"))
        .send()
        .await
    {
        if agg_resp.ok() {
            if let Ok(agg) = agg_resp.json::<serde_json::Value>().await {
                beer.average_score = agg["average_score"].as_f64();
                beer.rating_count = agg["unique_tasters"].as_i64().unwrap_or(0);
                beer.total_tastings = agg["total_tastings"].as_i64();
            }
        }
    }

    let brewery_resp =
        gloo_net::http::Request::get(&format!("/api/breweries/{}", beer.brewery_id))
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

    let brewery: BreweryInfo = if brewery_resp.ok() {
        brewery_resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {e}"))?
    } else {
        BreweryInfo {
            name: "Unknown".to_string(),
            country: None,
            city: None,
        }
    };

    Ok((beer, brewery))
}

async fn submit_rating(
    token: &str,
    beer_id: &str,
    score: i32,
    notes: Option<&str>,
) -> Result<(), String> {
    let mut body = serde_json::json!({
        "beer_id": beer_id,
        "score": score,
    });
    if let Some(n) = notes {
        body["notes"] = serde_json::Value::String(n.to_string());
    }

    // Use the new tastings API
    let resp = gloo_net::http::Request::post("/api/tastings")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        Ok(())
    } else {
        let data: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::json!({"error": "Failed to rate"}));
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to submit rating")
            .to_string())
    }
}
