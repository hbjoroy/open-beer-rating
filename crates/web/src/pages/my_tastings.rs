use leptos::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct TastingResponse {
    id: String,
    beer_id: String,
    beer_name: Option<String>,
    brewery_name: Option<String>,
    score: i32,
    serving_style: Option<String>,
    notes: Option<String>,
    location_name: Option<String>,
    session_name: Option<String>,
    tasted_at: String,
}

#[component]
pub fn MyTastingsPage(token: ReadSignal<Option<String>>) -> impl IntoView {
    let (tastings, set_tastings) = signal(Vec::<TastingResponse>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(Option::<String>::None);
    let (offset, set_offset) = signal(0i64);
    let (has_more, set_has_more) = signal(false);

    let load = move |off: i64| {
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        set_loading.set(true);
        leptos::task::spawn_local(async move {
            match fetch_tastings(&tok, 20, off).await {
                Ok(list) => {
                    set_has_more.set(list.len() == 20);
                    if off == 0 {
                        set_tastings.set(list);
                    } else {
                        set_tastings.update(|t| t.extend(list));
                    }
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        });
    };

    // Initial load
    {
        let load = load.clone();
        leptos::task::spawn_local(async move { load(0) });
    }

    view! {
        <div class="page my-tastings">
            <h2>"My Tastings 📝"</h2>

            {move || {
                if token.get().is_none() {
                    Some(view! { <p class="login-prompt">"Log in to see your tastings"</p> })
                } else {
                    None
                }
            }}

            {move || error.get().map(|e| view! { <div class="error">{e}</div> })}

            <div class="tastings-timeline">
                {move || {
                    let list = tastings.get();
                    if list.is_empty() && !loading.get() {
                        return view! { <p class="empty">"No tastings yet. Rate your first beer!"</p> }.into_any();
                    }
                    view! {
                        <div class="timeline">
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
                                view! {
                                    <div class="tasting-entry">
                                        <div class="tasting-header">
                                            <span class=format!("score-badge {score_class}")>
                                                {format!("{}/10", t.score)}
                                            </span>
                                            <div class="tasting-info">
                                                <strong>{t.beer_name.clone().unwrap_or_else(|| "Unknown".into())}</strong>
                                                <span class="brewery">{t.brewery_name.clone().unwrap_or_default()}</span>
                                            </div>
                                        </div>
                                        <div class="tasting-meta">
                                            <span class="date">{format_date(&t.tasted_at)}</span>
                                            {t.serving_style.as_ref().map(|s| view! {
                                                <span class="serving">{format_serving(s)}</span>
                                            })}
                                            {t.location_name.as_ref().map(|l| view! {
                                                <span class="location">"📍 " {l.clone()}</span>
                                            })}
                                            {t.session_name.as_ref().map(|s| view! {
                                                <span class="session">"📋 " {s.clone()}</span>
                                            })}
                                        </div>
                                        {t.notes.as_ref().map(|n| view! {
                                            <p class="tasting-notes">{n.clone()}</p>
                                        })}
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                }}

                {move || {
                    if loading.get() {
                        Some(view! { <p class="loading">"Loading..."</p> })
                    } else {
                        None
                    }
                }}

                {move || {
                    has_more.get().then(|| {
                        view! {
                            <button
                                class="btn-secondary load-more"
                                on:click=move |_| {
                                    let next = offset.get() + 20;
                                    set_offset.set(next);
                                    load(next);
                                }
                                disabled=move || loading.get()
                            >"Load More"</button>
                        }
                    })
                }}
            </div>
        </div>
    }
}

fn format_date(iso: &str) -> String {
    if iso.len() >= 10 {
        iso[..10].to_string()
    } else {
        iso.to_string()
    }
}

fn format_serving(style: &str) -> String {
    match style {
        "draft" => "🍺 Draft".to_string(),
        "bottle" => "🍾 Bottle".to_string(),
        "can" => "🥫 Can".to_string(),
        "cask" => "🪵 Cask".to_string(),
        "nitro" => "☁️ Nitro".to_string(),
        "crowler" => "Crowler".to_string(),
        "growler" => "Growler".to_string(),
        "taster" => "Taster".to_string(),
        _ => style.to_string(),
    }
}

async fn fetch_tastings(
    token: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<TastingResponse>, String> {
    let resp = gloo_net::http::Request::get(&format!(
        "/api/tastings?limit={limit}&offset={offset}"
    ))
    .header("Authorization", &format!("Bearer {token}"))
    .send()
    .await
    .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<TastingResponse>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to fetch tastings".to_string())
    }
}
