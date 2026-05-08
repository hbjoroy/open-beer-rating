use leptos::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct BeerResponse {
    id: String,
    name: String,
    style: Option<String>,
    abv: Option<f64>,
}

#[component]
pub fn BeerListPage() -> impl IntoView {
    let beers = leptos::prelude::LocalResource::new(|| fetch_beers());

    view! {
        <div class="page beer-list">
            <h2>"Beers"</h2>

            <Suspense fallback=move || view! { <p>"Loading beers..."</p> }>
                {move || {
                    beers.get().map(|result| {
                        match &*result {
                            Ok(beer_list) => {
                                if beer_list.is_empty() {
                                    view! { <p>"No beers yet. Be the first to add one!"</p> }.into_any()
                                } else {
                                    view! {
                                        <div class="beer-grid">
                                            {beer_list.iter().map(|beer| {
                                                view! {
                                                    <div class="beer-card">
                                                        <h3>{beer.name.clone()}</h3>
                                                        {beer.style.as_ref().map(|s| view! { <span class="style">{s.clone()}</span> })}
                                                        {beer.abv.map(|a| view! { <span class="abv">{format!("{:.1}%", a)}</span> })}
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
        </div>
    }
}

async fn fetch_beers() -> Result<Vec<BeerResponse>, String> {
    let resp = gloo_net::http::Request::get("/api/beers")
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<BeerResponse>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to fetch beers".to_string())
    }
}
