use leptos::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct BeerSearchResult {
    id: String,
    name: String,
    style: Option<String>,
    abv: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct TastingCreated {
    id: String,
    score: i32,
}

#[component]
pub fn RateBeerPage(
    token: ReadSignal<Option<String>>,
    active_session: ReadSignal<Option<ActiveSession>>,
) -> impl IntoView {
    let (search_query, set_search_query) = signal(String::new());
    let (search_results, set_search_results) = signal(Vec::<BeerSearchResult>::new());
    let (selected_beer, set_selected_beer) = signal(Option::<BeerSearchResult>::None);
    let (selected_score, set_selected_score) = signal(Option::<i32>::None);
    let (notes, set_notes) = signal(String::new());
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<String>::None);
    let (searching, set_searching) = signal(false);

    let on_search = move |_| {
        let query = search_query.get();
        if query.len() < 2 {
            return;
        }
        set_searching.set(true);
        leptos::task::spawn_local(async move {
            match search_beers(&query).await {
                Ok(results) => set_search_results.set(results),
                Err(e) => set_error.set(Some(e)),
            }
            set_searching.set(false);
        });
    };

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let beer = match selected_beer.get() {
            Some(b) => b,
            None => {
                set_error.set(Some("Please select a beer".into()));
                return;
            }
        };
        let score = match selected_score.get() {
            Some(s) => s,
            None => {
                set_error.set(Some("Please select a score".into()));
                return;
            }
        };
        let tok = match token.get() {
            Some(t) => t,
            None => {
                set_error.set(Some("Please log in to rate".into()));
                return;
            }
        };

        set_submitting.set(true);
        set_error.set(None);
        set_success.set(None);

        let notes_val = notes.get();
        let session_id = active_session.get().map(|s| s.id.clone());

        leptos::task::spawn_local(async move {
            match submit_tasting(
                &tok,
                &beer.id,
                score,
                if notes_val.is_empty() { None } else { Some(&notes_val) },
                session_id.as_deref(),
            )
            .await
            {
                Ok(_) => {
                    set_success.set(Some(format!(
                        "Rated {} — {}/10! 🎉",
                        beer.name, score
                    )));
                    set_selected_beer.set(None);
                    set_selected_score.set(None);
                    set_notes.set(String::new());
                    set_search_query.set(String::new());
                    set_search_results.set(Vec::new());
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_submitting.set(false);
        });
    };

    view! {
        <div class="page rate-beer">
            <h2>"Rate a Beer 🍺"</h2>

            {move || {
                if token.get().is_none() {
                    Some(view! { <p class="login-prompt">"Log in to rate beers"</p> })
                } else {
                    None
                }
            }}

            {move || success.get().map(|msg| view! { <div class="success">{msg}</div> })}
            {move || error.get().map(|e| view! { <div class="error">{e}</div> })}

            {move || active_session.get().map(|s| view! {
                <div class="active-session-banner">
                    "📋 Session: " <strong>{s.name.clone()}</strong>
                    " — tastings will be added to this session"
                </div>
            })}

            <div class="beer-search" style:display=move || if token.get().is_some() { "block" } else { "none" }>
                <div class="search-bar">
                    <input
                        type="text"
                        placeholder="Search for a beer..."
                        prop:value=move || search_query.get()
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                let query = search_query.get();
                                if query.len() >= 2 {
                                    set_searching.set(true);
                                    leptos::task::spawn_local(async move {
                                        match search_beers(&query).await {
                                            Ok(results) => set_search_results.set(results),
                                            Err(e) => set_error.set(Some(e)),
                                        }
                                        set_searching.set(false);
                                    });
                                }
                            }
                        }
                    />
                    <button type="button" on:click=on_search disabled=move || searching.get()>
                        {move || if searching.get() { "..." } else { "Search" }}
                    </button>
                </div>

                {move || {
                    let results = search_results.get();
                    let sel = selected_beer.get();
                    if !results.is_empty() && sel.is_none() {
                        Some(view! {
                            <div class="search-results">
                                {results.iter().map(|beer| {
                                    let b = beer.clone();
                                    view! {
                                        <div
                                            class="search-result-item"
                                            on:click=move |_| {
                                                set_selected_beer.set(Some(b.clone()));
                                                set_search_results.set(Vec::new());
                                            }
                                        >
                                            <strong>{beer.name.clone()}</strong>
                                            {beer.style.as_ref().map(|s| format!(" • {s}"))}
                                            {beer.abv.map(|a| format!(" • {:.1}%", a))}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        })
                    } else {
                        None
                    }
                }}

                {move || selected_beer.get().map(|beer| view! {
                    <div class="selected-beer">
                        <span>"Selected: " <strong>{beer.name.clone()}</strong></span>
                        <button type="button" class="btn-text" on:click=move |_| set_selected_beer.set(None)>"✕ Change"</button>
                    </div>
                })}
            </div>

            <form
                on:submit=on_submit
                style:display=move || if token.get().is_some() && selected_beer.get().is_some() { "block" } else { "none" }
            >
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

                <button type="submit" class="btn-primary" disabled=move || submitting.get()>
                    {move || if submitting.get() { "Submitting..." } else { "Submit Tasting" }}
                </button>
            </form>
        </div>
    }
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub id: String,
    pub name: String,
}

async fn search_beers(query: &str) -> Result<Vec<BeerSearchResult>, String> {
    let resp = gloo_net::http::Request::get(&format!("/api/beers?search={}", query))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<BeerSearchResult>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to search beers".to_string())
    }
}

async fn submit_tasting(
    token: &str,
    beer_id: &str,
    score: i32,
    notes: Option<&str>,
    session_id: Option<&str>,
) -> Result<TastingCreated, String> {
    let mut body = serde_json::json!({
        "beer_id": beer_id,
        "score": score,
    });
    if let Some(n) = notes {
        body["notes"] = serde_json::Value::String(n.to_string());
    }
    if let Some(sid) = session_id {
        body["session_id"] = serde_json::Value::String(sid.to_string());
    }

    let resp = gloo_net::http::Request::post("/api/tastings")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<TastingCreated>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        let data: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::json!({"error": "Failed to submit"}));
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to submit tasting")
            .to_string())
    }
}
