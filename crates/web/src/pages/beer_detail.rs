use crate::components::score_badge::ScoreBadge;
use leptos::prelude::*;
use serde::Deserialize;

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
    let _ = on_back;
    let (refresh, set_refresh) = signal(0u32);
    let fetch_id = beer_id.clone();
    let detail = LocalResource::new(move || {
        refresh.get();
        let id = fetch_id.clone();
        async move { fetch_beer_with_brewery(&id).await }
    });
    let tasting_url = format!("/tastings/new?beer={beer_id}");
    let history_url = format!("/tastings?beer={beer_id}");
    let login_url = format!(
        "/login?next={}",
        super::beer_list::query_encode(&tasting_url)
    );
    view! {
        <div class="page beer-detail">
            <a class="back-link" href="/beers">"Back to beers"</a>
            <Suspense fallback=move || view! { <p role="status">"Loading beer…"</p> }>
                {move || detail.get().map(|result| match &*result {
                    Ok((beer, brewery)) => view! {
                        <article class="detail-card">
                            <h2>{beer.name.clone()}</h2>
                            <p class="brewery-info">"by "<strong>{brewery.name.clone()}</strong>
                                {brewery.city.as_ref().map(|city| format!(", {city}"))}
                                {brewery.country.as_ref().map(|country| format!(" ({country})"))}
                            </p>
                            <div class="beer-meta">
                                {beer.style.clone().map(|style| view! { <span class="style">{style}</span> })}
                                {beer.abv.map(|abv| view! { <span class="abv">{format!("{abv:.1}% ABV")}</span> })}
                            </div>
                            {beer.description.clone().map(|description| view! { <p class="description">{description}</p> })}
                            <div class="aggregate-score">
                                {beer.average_score.map(|average| view! {
                                    <ScoreBadge score=average.round() as i32 />
                                    <span>{format!("{average:.1}/10 average")}</span>
                                })}
                                <span>{format!("{} tasters / {} tastings", beer.rating_count, beer.total_tastings.unwrap_or(beer.rating_count))}</span>
                                {beer.average_score.is_none().then(|| view! { <p>"No scores yet. Record the first tasting."</p> })}
                            </div>
                        </article>
                    }.into_any(),
                    Err(error) => view! { <div><p class="error" role="alert">{error.clone()}</p><button on:click=move |_| set_refresh.update(|n| *n += 1)>"Retry"</button></div> }.into_any(),
                })}
            </Suspense>
            <div class="rate-section">
                <h3>"Your tastings"</h3>
                <div class="detail-actions">
                    <a class="btn-primary" href=tasting_url>"Record a tasting"</a>
                    <a href=history_url>"View my tastings of this beer"</a>
                </div>
                <Show when=move || token.get().is_none()>
                    <p class="login-prompt"><a href=login_url.clone()>"Log in to save a tasting"</a></p>
                </Show>
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

    let mut beer: BeerDetail = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;

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

    let brewery_resp = gloo_net::http::Request::get(&format!("/api/breweries/{}", beer.brewery_id))
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
