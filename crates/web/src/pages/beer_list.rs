use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use serde::Deserialize;

pub(super) const PAGE_SIZE: usize = 20;

pub(super) fn page_number(raw: Option<String>) -> usize {
    raw.and_then(|value| value.parse::<usize>().ok())
        .filter(|page| *page > 0 && *page <= (i64::MAX as usize / PAGE_SIZE))
        .unwrap_or(1)
}

// Encode UTF-8 bytes, including reserved query characters, without browser-only APIs.
pub(super) fn query_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

pub(super) fn list_url(path: &str, key: &str, value: &str, page: usize) -> String {
    if value.is_empty() {
        format!("{path}?page={page}")
    } else {
        format!("{path}?{key}={}&page={page}", query_encode(value))
    }
}

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
    let _ = (on_view_beer, on_add_beer);
    let query = use_query_map();
    let page = Memo::new(move |_| page_number(query.with(|q| q.get("page"))));
    let search = Memo::new(move |_| query.with(|q| q.get("q")).unwrap_or_default());
    let (draft, set_draft) = signal(search.get_untracked());
    Effect::new(move |_| set_draft.set(search.get()));
    let (refresh, set_refresh) = signal(0u32);
    let beers = LocalResource::new(move || {
        refresh.get();
        fetch_beers(search.get(), page.get())
    });
    let navigate = use_navigate();
    view! {
        <div class="page beer-list">
            <div class="page-header">
                <h2>"Beers"</h2>
                <Show when=move || token.get().is_some()>
                    <a class="btn-primary" href="/beers/new">"+ Add Beer"</a>
                </Show>
            </div>
            <form class="search-form" on:submit=move |ev| {
                ev.prevent_default();
                navigate(&list_url("/beers", "q", draft.get().trim(), 1), Default::default());
            }>
                <label for="beer-search">"Search beers"</label>
                <input id="beer-search" type="search" prop:value=move || draft.get()
                    on:input=move |ev| set_draft.set(event_target_value(&ev)) />
                <button type="submit">"Search"</button>
                <a href="/beers">"Clear search"</a>
            </form>
            <Suspense fallback=move || view! { <p role="status">"Loading beers…"</p> }>
                {move || beers.get().map(|result| match &*result {
                    Ok(list) => {
                        let has_next = list.len() > PAGE_SIZE;
                        view! {
                            <div>
                                {if list.is_empty() {
                                    view! { <p class="empty">{if page.get() > 1 { "No beers on this page. Return to the previous page." } else if search.get().is_empty() { "No beers yet. Add the first beer to the catalog." } else { "No beers match your search. Try another name or clear the search." }}</p> }.into_any()
                                } else {
                                    view! { <div class="beer-grid">
                                        {list.iter().take(PAGE_SIZE).map(|beer| view! {
                                            <article class="beer-card">
                                                <h3><a href=format!("/beers/{}", beer.id)>{beer.name.clone()}</a></h3>
                                                {beer.style.clone().map(|style| view! { <span class="style">{style}</span> })}
                                                {beer.abv.map(|abv| view! { <span class="abv">{format!("{abv:.1}% ABV")}</span> })}
                                            </article>
                                        }).collect_view()}
                                    </div> }.into_any()
                                }}
                                <nav class="pagination" aria-label="Beer pages">
                                    {(page.get() > 1).then(|| view! { <a href=list_url("/beers", "q", &search.get(), page.get() - 1)>"Previous"</a> })}
                                    <span>{format!("Page {}", page.get())}</span>
                                    {has_next.then(|| view! { <a href=list_url("/beers", "q", &search.get(), page.get() + 1)>"Next"</a> })}
                                </nav>
                            </div>
                        }.into_any()
                    },
                    Err(error) => view! { <div><p class="error" role="alert">{error.clone()}</p><button on:click=move |_| set_refresh.update(|n| *n += 1)>"Retry"</button></div> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

async fn fetch_beers(search: String, page: usize) -> Result<Vec<BeerResponse>, String> {
    let url = format!(
        "/api/beers?limit={}&offset={}&search={}",
        PAGE_SIZE + 1,
        (page - 1) * PAGE_SIZE,
        query_encode(&search)
    );
    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|_| "Could not reach the server. Please try again.".to_string())?;
    if !response.ok() {
        return Err("Could not load beers. Please try again.".into());
    }
    response
        .json()
        .await
        .map_err(|_| "Could not read the beer catalog response.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_round_trip_preserves_reserved_and_unicode_characters() {
        assert_eq!(
            list_url("/beers", "q", "Mikk øl & IPA+", 2),
            "/beers?q=Mikk%20%C3%B8l%20%26%20IPA%2B&page=2"
        );
    }
    #[test]
    fn invalid_or_overflowing_pages_start_at_first_page() {
        for value in ["0", "-1", "no", "18446744073709551615"] {
            assert_eq!(page_number(Some(value.into())), 1);
        }
        assert_eq!(page_number(Some("3".into())), 3);
    }
}
