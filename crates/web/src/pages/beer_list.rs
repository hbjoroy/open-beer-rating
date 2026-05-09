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
pub fn BeerListPage(
    token: ReadSignal<Option<String>>,
    on_view_beer: impl Fn(String) + Send + Sync + 'static,
    on_add_beer: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let on_view_beer = std::sync::Arc::new(on_view_beer);
    let on_add_beer = std::sync::Arc::new(on_add_beer);

    let beers = leptos::prelude::LocalResource::new(|| fetch_beers());

    let on_add = on_add_beer.clone();

    view! {
        <div class="page beer-list">
            <div class="page-header">
                <h2>"Beers"</h2>
                {move || {
                    let on_add = on_add.clone();
                    token.get().is_some().then(move || {
                        view! {
                            <button class="btn-primary" on:click=move |_| on_add()>"+ Add Beer"</button>
                        }
                    })
                }}
            </div>

            <Suspense fallback=move || view! { <p>"Loading beers..."</p> }>
                {move || {
                    let on_view = on_view_beer.clone();
                    beers.get().map(|result| {
                        match &*result {
                            Ok(beer_list) => {
                                if beer_list.is_empty() {
                                    view! { <p>"No beers yet. Be the first to add one!"</p> }.into_any()
                                } else {
                                    let on_view = on_view.clone();
                                    view! {
                                        <div class="beer-grid">
                                            {beer_list.iter().map(|beer| {
                                                let on_view = on_view.clone();
                                                let id = beer.id.clone();
                                                view! {
                                                    <div class="beer-card clickable" on:click=move |_| on_view(id.clone())>
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
