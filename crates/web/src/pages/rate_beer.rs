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

#[derive(Debug, Clone, Deserialize)]
struct LocationItem {
    id: String,
    name: String,
    location_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct SessionItem {
    id: String,
    name: String,
    join_code: String,
    ended_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BreweryOption {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CreatedBeerResponse {
    id: String,
    name: String,
}

const SERVING_STYLES: &[(&str, &str)] = &[
    ("draft", "🍺 Draft"),
    ("bottle", "🍾 Bottle"),
    ("can", "🥫 Can"),
    ("cask", "🪵 Cask"),
    ("crowler", "Crowler"),
    ("growler", "Growler"),
    ("nitro", "☁️ Nitro"),
    ("taster", "Taster"),
    ("other", "Other"),
];

#[component]
pub fn RateBeerPage(
    token: ReadSignal<Option<String>>,
    active_session: ReadSignal<Option<ActiveSession>>,
    set_active_session: WriteSignal<Option<ActiveSession>>,
) -> impl IntoView {
    let (search_query, set_search_query) = signal(String::new());
    let (search_results, set_search_results) = signal(Vec::<BeerSearchResult>::new());
    let (selected_beer, set_selected_beer) = signal(Option::<BeerSearchResult>::None);
    let (selected_score, set_selected_score) = signal(Option::<i32>::None);
    let (selected_serving, set_selected_serving) = signal(get_last_serving());
    let (notes, set_notes) = signal(String::new());
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<String>::None);
    let (searching, set_searching) = signal(false);

    // Location state — restore last used
    let (locations, set_locations) = signal(Vec::<LocationItem>::new());
    let (selected_location, set_selected_location) = signal(get_last_location());

    // Session state
    let (sessions, set_sessions) = signal(Vec::<SessionItem>::new());
    let (show_create_session, set_show_create_session) = signal(false);
    let (new_session_name, set_new_session_name) = signal(String::new());
    let (creating_session, set_creating_session) = signal(false);

    // Inline beer creation state
    let (show_quick_add, set_show_quick_add) = signal(false);
    let (quick_beer_name, set_quick_beer_name) = signal(String::new());
    let (quick_brewery_id, set_quick_brewery_id) = signal(String::new());
    let (quick_new_brewery, set_quick_new_brewery) = signal(false);
    let (quick_brewery_name, set_quick_brewery_name) = signal(String::new());
    let (quick_style, set_quick_style) = signal(String::new());
    let (quick_abv, set_quick_abv) = signal(String::new());
    let (adding_beer, set_adding_beer) = signal(false);
    let (breweries, set_breweries) = signal(Vec::<BreweryOption>::new());

    // Load locations and sessions when token is available
    {
        let tok = token.get_untracked();
        if let Some(tok) = tok {
            let tok2 = tok.clone();
            leptos::task::spawn_local(async move {
                if let Ok(locs) = fetch_locations(&tok).await {
                    set_locations.set(locs);
                }
            });
            leptos::task::spawn_local(async move {
                if let Ok(sess) = fetch_active_sessions(&tok2).await {
                    set_sessions.set(sess);
                }
            });
        }
    }

    let on_search = move |_| {
        let query = search_query.get();
        if query.len() < 2 {
            return;
        }
        set_searching.set(true);
        set_show_quick_add.set(false);
        leptos::task::spawn_local(async move {
            match search_beers(&query).await {
                Ok(results) => {
                    set_search_results.set(results);
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_searching.set(false);
        });
    };

    let on_create_session = move |_| {
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        let name = new_session_name.get();
        if name.is_empty() {
            return;
        }
        set_creating_session.set(true);
        leptos::task::spawn_local(async move {
            match create_session(&tok, &name).await {
                Ok(sess) => {
                    set_active_session.set(Some(ActiveSession {
                        id: sess.id.clone(),
                        name: sess.name.clone(),
                    }));
                    set_sessions.update(|list| list.insert(0, sess));
                    set_new_session_name.set(String::new());
                    set_show_create_session.set(false);
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_creating_session.set(false);
        });
    };

    let on_select_session = move |id: String, name: String| {
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        let id2 = id.clone();
        let name2 = name.clone();
        leptos::task::spawn_local(async move {
            let _ = join_session(&tok, &id2).await;
            set_active_session.set(Some(ActiveSession {
                id: id2,
                name: name2,
            }));
        });
    };

    // Quick add beer handler
    let on_quick_add_beer = move |_| {
        let tok = match token.get() {
            Some(t) => t,
            None => return,
        };
        let name = quick_beer_name.get();
        if name.is_empty() {
            set_error.set(Some("Beer name is required".into()));
            return;
        }

        set_adding_beer.set(true);
        set_error.set(None);
        let is_new_brewery = quick_new_brewery.get();
        let brewery_id = quick_brewery_id.get();
        let brewery_name = quick_brewery_name.get();
        let style_val = quick_style.get();
        let abv_val = quick_abv.get();

        leptos::task::spawn_local(async move {
            // Resolve brewery
            let bid = if is_new_brewery {
                if brewery_name.is_empty() {
                    set_error.set(Some("Brewery name is required".into()));
                    set_adding_beer.set(false);
                    return;
                }
                match create_brewery_api(&tok, &brewery_name).await {
                    Ok(id) => id,
                    Err(e) => {
                        set_error.set(Some(e));
                        set_adding_beer.set(false);
                        return;
                    }
                }
            } else {
                if brewery_id.is_empty() {
                    set_error.set(Some("Please select a brewery".into()));
                    set_adding_beer.set(false);
                    return;
                }
                brewery_id
            };

            let abv_parsed = if abv_val.is_empty() {
                None
            } else {
                abv_val.parse::<f64>().ok()
            };

            match create_beer_api(
                &tok,
                &bid,
                &name,
                if style_val.is_empty() {
                    None
                } else {
                    Some(&style_val)
                },
                abv_parsed,
            )
            .await
            {
                Ok(created) => {
                    // Auto-select the newly created beer
                    set_selected_beer.set(Some(BeerSearchResult {
                        id: created.id,
                        name: created.name,
                        style: if style_val.is_empty() {
                            None
                        } else {
                            Some(style_val)
                        },
                        abv: abv_parsed,
                    }));
                    set_show_quick_add.set(false);
                    set_quick_beer_name.set(String::new());
                    set_quick_brewery_id.set(String::new());
                    set_quick_new_brewery.set(false);
                    set_quick_brewery_name.set(String::new());
                    set_quick_style.set(String::new());
                    set_quick_abv.set(String::new());
                    set_search_results.set(Vec::new());
                    set_search_query.set(String::new());
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_adding_beer.set(false);
        });
    };

    // Show quick add and pre-fill with search query
    let on_show_quick_add = move |_| {
        set_show_quick_add.set(true);
        set_quick_beer_name.set(search_query.get());
        // Load breweries for the selector
        if let Some(tok) = token.get() {
            leptos::task::spawn_local(async move {
                if let Ok(list) = fetch_breweries_api().await {
                    set_breweries.set(list);
                }
                let _ = tok;
            });
        }
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
        let location_id = selected_location.get();
        let serving = selected_serving.get();

        // Persist serving style and location for next time
        if let Some(ref s) = serving {
            save_last_serving(s);
        }
        if let Some(ref l) = location_id {
            save_last_location(l);
        }

        leptos::task::spawn_local(async move {
            match submit_tasting(
                &tok,
                &beer.id,
                score,
                serving.as_deref(),
                if notes_val.is_empty() {
                    None
                } else {
                    Some(&notes_val)
                },
                location_id.as_deref(),
                session_id.as_deref(),
            )
            .await
            {
                Ok(_) => {
                    set_success.set(Some(format!("Rated {} — {}/10! 🎉", beer.name, score)));
                    set_selected_beer.set(None);
                    set_selected_score.set(None);
                    // Keep serving style & location for next rating
                    set_notes.set(String::new());
                    set_search_query.set(String::new());
                    set_search_results.set(Vec::new());
                    // Auto-dismiss success after 3 seconds
                    leptos::task::spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(3_000).await;
                        set_success.set(None);
                    });
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

            // Session selector
            <div class="session-picker" style:display=move || if token.get().is_some() { "block" } else { "none" }>
                {move || {
                    if let Some(s) = active_session.get() {
                        view! {
                            <div class="active-session-banner">
                                "📋 Session: " <strong>{s.name.clone()}</strong>
                                <button type="button" class="btn-text" on:click=move |_| set_active_session.set(None)>"✕ Leave"</button>
                            </div>
                        }.into_any()
                    } else {
                        let sess = sessions.get();
                        view! {
                            <div class="session-select-row">
                                <label>"Session (optional):"</label>
                                {if !sess.is_empty() {
                                    let items = sess.clone();
                                    view! {
                                        <div class="session-chips">
                                            {items.iter().map(|s| {
                                                let sid = s.id.clone();
                                                let sname = s.name.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="chip"
                                                        on:click=move |_| on_select_session(sid.clone(), sname.clone())
                                                    >{s.name.clone()}</button>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <span class="muted">"No active sessions"</span> }.into_any()
                                }}
                                <button type="button" class="btn-text" on:click=move |_| set_show_create_session.update(|v| *v = !*v)>
                                    {move || if show_create_session.get() { "Cancel" } else { "+ New" }}
                                </button>
                            </div>
                            {move || show_create_session.get().then(|| view! {
                                <div class="inline-create">
                                    <input
                                        type="text"
                                        placeholder="Session name..."
                                        prop:value=move || new_session_name.get()
                                        on:input=move |ev| set_new_session_name.set(event_target_value(&ev))
                                    />
                                    <button type="button" on:click=on_create_session disabled=move || creating_session.get()>
                                        {move || if creating_session.get() { "..." } else { "Create" }}
                                    </button>
                                </div>
                            })}
                        }.into_any()
                    }
                }}
            </div>

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
                                    set_show_quick_add.set(false);
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

                // "Not found? Quick-add" button
                {move || {
                    let results = search_results.get();
                    let sel = selected_beer.get();
                    let query = search_query.get();
                    if sel.is_none() && query.len() >= 2 && !searching.get() && !show_quick_add.get() {
                        Some(view! {
                            <button
                                type="button"
                                class="btn-text quick-add-trigger"
                                on:click=on_show_quick_add
                            >
                                {if results.is_empty() {
                                    "Beer not in database? Add it quick →"
                                } else {
                                    "Not the right one? Add a new beer →"
                                }}
                            </button>
                        })
                    } else {
                        None
                    }
                }}

                // Inline quick-add beer form
                {move || show_quick_add.get().then(|| view! {
                    <div class="quick-add-form">
                        <h4>"Quick Add Beer"</h4>
                        <div class="form-group">
                            <input
                                type="text"
                                placeholder="Beer name"
                                prop:value=move || quick_beer_name.get()
                                on:input=move |ev| set_quick_beer_name.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="form-group">
                            {move || {
                                if quick_new_brewery.get() {
                                    view! {
                                        <div class="inline-create">
                                            <input
                                                type="text"
                                                placeholder="New brewery name"
                                                prop:value=move || quick_brewery_name.get()
                                                on:input=move |ev| set_quick_brewery_name.set(event_target_value(&ev))
                                            />
                                            <button type="button" class="btn-text" on:click=move |_| set_quick_new_brewery.set(false)>"Cancel"</button>
                                        </div>
                                    }.into_any()
                                } else {
                                    let brew_list = breweries.get();
                                    view! {
                                        <div class="inline-create">
                                            <select
                                                on:change=move |ev| set_quick_brewery_id.set(event_target_value(&ev))
                                            >
                                                <option value="">"— Select brewery —"</option>
                                                {brew_list.iter().map(|b| {
                                                    let id = b.id.clone();
                                                    let name = b.name.clone();
                                                    view! { <option value=id>{name}</option> }
                                                }).collect::<Vec<_>>()}
                                            </select>
                                            <button type="button" class="btn-text" on:click=move |_| set_quick_new_brewery.set(true)>"+ New"</button>
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <input
                                    type="text"
                                    placeholder="Style (optional)"
                                    prop:value=move || quick_style.get()
                                    on:input=move |ev| set_quick_style.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="form-group">
                                <input
                                    type="number"
                                    step="0.1"
                                    placeholder="ABV %"
                                    prop:value=move || quick_abv.get()
                                    on:input=move |ev| set_quick_abv.set(event_target_value(&ev))
                                />
                            </div>
                        </div>
                        <div class="quick-add-actions">
                            <button type="button" class="btn-primary" on:click=on_quick_add_beer disabled=move || adding_beer.get()>
                                {move || if adding_beer.get() { "Adding..." } else { "Add & Select" }}
                            </button>
                            <button type="button" class="btn-text" on:click=move |_| set_show_quick_add.set(false)>"Cancel"</button>
                        </div>
                    </div>
                })}

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

                <label class="form-label">"Serving style"</label>
                <div class="serving-selector">
                    {SERVING_STYLES.iter().map(|(val, label)| {
                        let v = val.to_string();
                        let v2 = val.to_string();
                        let l = *label;
                        view! {
                            <button
                                type="button"
                                class=move || {
                                    if selected_serving.get().as_deref() == Some(&v) {
                                        "chip selected"
                                    } else {
                                        "chip"
                                    }
                                }
                                on:click=move |_| {
                                    if selected_serving.get().as_deref() == Some(&v2) {
                                        set_selected_serving.set(None);
                                    } else {
                                        set_selected_serving.set(Some(v2.clone()));
                                    }
                                }
                            >
                                {l}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                // Location picker
                <div class="form-group">
                    <label>"Location (optional)"</label>
                    <select
                        prop:value=move || selected_location.get().unwrap_or_default()
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            set_selected_location.set(if val.is_empty() { None } else { Some(val) });
                        }
                    >
                        <option value="">"— No location —"</option>
                        {move || locations.get().iter().map(|loc| {
                            view! {
                                <option value={loc.id.clone()}>
                                    {format!("{} ({})", loc.name, loc.location_type)}
                                </option>
                            }
                        }).collect::<Vec<_>>()}
                    </select>
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

/// Store the active session in localStorage so it survives page refreshes.
pub fn persist_active_session(session: &ActiveSession) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("open_tappd_session_id", &session.id);
        let _ = storage.set_item("open_tappd_session_name", &session.name);
    }
}

/// Clear the active session from localStorage.
pub fn clear_persisted_session() {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item("open_tappd_session_id");
        let _ = storage.remove_item("open_tappd_session_name");
    }
}

/// Restore the active session from localStorage (returns None if not stored).
pub fn restore_persisted_session() -> Option<ActiveSession> {
    let storage = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()?;
    let id = storage.get_item("open_tappd_session_id").ok().flatten()?;
    let name = storage.get_item("open_tappd_session_name").ok().flatten()?;
    if id.is_empty() {
        return None;
    }
    Some(ActiveSession { id, name })
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

async fn fetch_locations(token: &str) -> Result<Vec<LocationItem>, String> {
    let resp = gloo_net::http::Request::get("/api/locations?limit=100")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<LocationItem>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Ok(Vec::new())
    }
}

async fn fetch_active_sessions(token: &str) -> Result<Vec<SessionItem>, String> {
    let resp = gloo_net::http::Request::get("/api/tasting-sessions?active_only=true&limit=50")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<SessionItem>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Ok(Vec::new())
    }
}

async fn create_session(token: &str, name: &str) -> Result<SessionItem, String> {
    let body = serde_json::json!({
        "name": name,
        "visibility": "public",
    });

    let resp = gloo_net::http::Request::post("/api/tasting-sessions")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<SessionItem>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to create session")
            .to_string())
    }
}

async fn join_session(token: &str, session_id: &str) -> Result<(), String> {
    let resp = gloo_net::http::Request::post(&format!("/api/tasting-sessions/{session_id}/join"))
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() || resp.status() == 200 {
        Ok(())
    } else {
        Err("Failed to join session".to_string())
    }
}

async fn submit_tasting(
    token: &str,
    beer_id: &str,
    score: i32,
    serving_style: Option<&str>,
    notes: Option<&str>,
    location_id: Option<&str>,
    session_id: Option<&str>,
) -> Result<TastingCreated, String> {
    let mut body = serde_json::json!({
        "beer_id": beer_id,
        "score": score,
    });
    if let Some(s) = serving_style {
        body["serving_style"] = serde_json::Value::String(s.to_string());
    }
    if let Some(n) = notes {
        body["notes"] = serde_json::Value::String(n.to_string());
    }
    if let Some(lid) = location_id {
        body["location_id"] = serde_json::Value::String(lid.to_string());
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

// --- localStorage helpers for remembering user preferences ---

fn get_last_serving() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("open_tappd_last_serving").ok())
        .flatten()
        .filter(|v| !v.is_empty())
}

fn save_last_serving(serving: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("open_tappd_last_serving", serving);
    }
}

fn get_last_location() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("open_tappd_last_location").ok())
        .flatten()
        .filter(|v| !v.is_empty())
}

fn save_last_location(location_id: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("open_tappd_last_location", location_id);
    }
}

// --- API helpers for inline beer creation ---

async fn fetch_breweries_api() -> Result<Vec<BreweryOption>, String> {
    let resp = gloo_net::http::Request::get("/api/breweries")
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<Vec<BreweryOption>>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        Err("Failed to fetch breweries".to_string())
    }
}

async fn create_brewery_api(token: &str, name: &str) -> Result<String, String> {
    let body = serde_json::json!({ "name": name });

    let resp = gloo_net::http::Request::post("/api/breweries")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
        data["id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No ID in response".to_string())
    } else {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to create brewery")
            .to_string())
    }
}

async fn create_beer_api(
    token: &str,
    brewery_id: &str,
    name: &str,
    style: Option<&str>,
    abv: Option<f64>,
) -> Result<CreatedBeerResponse, String> {
    let mut body = serde_json::json!({
        "brewery_id": brewery_id,
        "name": name,
    });
    if let Some(s) = style {
        body["style"] = serde_json::Value::String(s.to_string());
    }
    if let Some(a) = abv {
        body["abv"] = serde_json::json!(a);
    }

    let resp = gloo_net::http::Request::post("/api/beers")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .body(body.to_string())
        .map_err(|e| format!("Request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if resp.ok() {
        resp.json::<CreatedBeerResponse>()
            .await
            .map_err(|e| format!("Parse error: {e}"))
    } else {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to create beer")
            .to_string())
    }
}
