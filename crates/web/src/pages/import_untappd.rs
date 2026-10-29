use leptos::prelude::*;
use leptos_router::components::A;
use serde::Deserialize;
use std::sync::Arc;
use wasm_bindgen_futures::JsFuture;

const MAX_BYTES: f64 = 16.0 * 1024.0 * 1024.0;

#[derive(Clone, Deserialize)]
struct CatalogCount {
    created: usize,
    reused: usize,
}
#[derive(Clone, Deserialize)]
struct Report {
    rows: usize,
    imported: usize,
    already_imported: usize,
    duplicate_rows: usize,
    unrated_checkins: Vec<i64>,
    rounded_scores: usize,
    photo_links: usize,
    breweries: CatalogCount,
    beers: CatalogCount,
    locations: CatalogCount,
}
#[derive(Clone, Deserialize)]
struct ImportResponse {
    report: Report,
    preview_token: Option<String>,
    committed: bool,
}

#[component]
pub fn ImportUntappdPage(token: ReadSignal<Option<String>>) -> impl IntoView {
    let file = RwSignal::new(None::<Arc<String>>);
    let filename = RwSignal::new(String::new());
    let result = RwSignal::new(None::<ImportResponse>);
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let confirmed = RwSignal::new(false);
    let generation = RwSignal::new(0_u64);
    Effect::new(move |_| {
        token.track();
        generation.update(|g| *g += 1);
        file.set(None);
        filename.set(String::new());
        result.set(None);
        error.set(None);
        busy.set(false);
        confirmed.set(false);
    });
    let select_file = move |ev: leptos::ev::Event| {
        if busy.get_untracked() {
            return;
        }
        let input = event_target::<web_sys::HtmlInputElement>(&ev);
        let selected = input.files().and_then(|f| f.get(0));
        input.set_value("");
        let Some(selected) = selected else {
            return;
        };
        generation.update(|g| *g += 1);
        let version = generation.get_untracked();
        file.set(None);
        result.set(None);
        error.set(None);
        confirmed.set(false);
        filename.set(selected.name());
        if selected.size() > MAX_BYTES || selected.size() == 0.0 {
            error.set(Some(
                "Choose a non-empty Untappd JSON file, up to 16 MiB.".into(),
            ));
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let text = JsFuture::from(selected.text()).await;
            if generation.try_get_untracked() != Some(version) {
                return;
            }
            match text.ok().and_then(|v| v.as_string()) {
                Some(text) => file.set(Some(Arc::new(text))),
                None => error.set(Some("Could not read this file. Choose it again.".into())),
            }
            busy.set(false);
        });
    };
    let submit = Callback::new(move |commit: bool| {
        if busy.get_untracked() || (commit && !confirmed.get_untracked()) {
            return;
        }
        let (Some(auth), Some(body)) = (token.get_untracked(), file.get_untracked()) else {
            return;
        };
        let receipt = if commit {
            result.get_untracked().and_then(|r| r.preview_token)
        } else {
            None
        };
        if commit && receipt.is_none() {
            return;
        }
        let version = generation.get_untracked();
        busy.set(true);
        error.set(None);
        result.set(None);
        confirmed.set(false);
        leptos::task::spawn_local(async move {
            let response = send(&auth, body.as_str(), receipt.as_deref()).await;
            if generation.try_get_untracked() != Some(version)
                || token.try_get_untracked().flatten().as_deref() != Some(&auth)
            {
                return;
            }
            match response {
                Ok(data) => {
                    if data.committed {
                        file.set(None);
                    }
                    result.set(Some(data));
                }
                Err(message) => error.set(Some(message)),
            }
            busy.set(false);
        });
    });
    view! {
        <div class="page import-page">
            <A href="/account">"← My Profile"</A>
            <h1>"Import from Untappd"</h1>
            <p>"Bring your tasting history into your current Open Drinks account. Preview the changes before saving anything."</p>
            <Show when=move || token.get().is_some() fallback=|| view! {
                <p><A href="/login?next=/account/import">"Sign in to import your history"</A></p>
            }>
                <section class="import-panel" aria-labelledby="import-file-heading">
                    <h2 id="import-file-heading">"1. Choose your export"</h2>
                    <label for="untappd-file">"Untappd JSON export (up to 16 MiB, 10,000 check-ins)"</label>
                    <input id="untappd-file" type="file" accept=".json,application/json" on:change=select_file disabled=move || busy.get() />
                    <p class="muted">{move || filename.get()}</p>
                    <p>"Dates, ratings, notes, serving styles, beers, breweries and venues are imported. Existing Untappd check-ins are skipped; your later edits are kept."</p>
                    <p>"Beers, breweries and venues form a shared catalog. Your notes and tasting history retain their existing access rules."</p>
                    <button type="button" class="btn-primary" disabled=move || busy.get() || file.get().is_none() on:click=move |_| submit.run(false)>"Preview import"</button>
                </section>
                <Show when=move || busy.get()><p role="status">"Processing your file… Large exports may take a little while. Keep this page open."</p></Show>
                {move || error.get().map(|e| view! { <div role="alert"><p>{e}</p><p>"Preview the file again to retry safely. Check-ins already imported will be skipped."</p></div> })}
                {move || result.get().map(|response| {
                    let r = response.report;
                    let committed = response.committed;
                    let unrated = r.unrated_checkins.len();
                    let can_import = r.imported > 0;
                    view! {
                        <section class="import-panel" aria-labelledby="import-report-heading">
                            <h2 id="import-report-heading">{if committed { "Import complete" } else { "2. Review your import" }}</h2>
                            <p role="status">{format!("{} check-ins in this file. {} {}. {} already imported. {} duplicate rows skipped.", r.rows, r.imported, if committed { "imported" } else { "ready to import" }, r.already_imported, r.duplicate_rows)}</p>
                            <table class="import-summary"><caption>"Shared catalog entries used by this import"</caption>
                                <thead><tr><th scope="col">"Catalog"</th><th scope="col">"New"</th><th scope="col">"Reused"</th></tr></thead>
                                <tbody>
                                    <tr><th scope="row">"Beers"</th><td>{r.beers.created}</td><td>{r.beers.reused}</td></tr>
                                    <tr><th scope="row">"Breweries"</th><td>{r.breweries.created}</td><td>{r.breweries.reused}</td></tr>
                                    <tr><th scope="row">"Venues"</th><td>{r.locations.created}</td><td>{r.locations.reused}</td></tr>
                                </tbody>
                            </table>
                            <p>{format!("Ratings are doubled to 0–10. {} ratings need rounding to the nearest integer; their original values are preserved in your data export.", r.rounded_scores)}</p>
                            <p>{format!("{} photo links are preserved privately. The images themselves are not downloaded. Social activity, friends, badges and global ratings are not imported.", r.photo_links)}</p>
                            {(unrated > 0).then(|| view! {
                                <details><summary>{format!("{unrated} check-ins without a rating will be skipped")}</summary>
                                    <p>"Open Drinks currently requires a rating. These check-ins are not turned into zero-star ratings. Untappd IDs:"</p>
                                    <p>{r.unrated_checkins.iter().map(i64::to_string).collect::<Vec<_>>().join(", ")}</p>
                                </details>
                            })}
                            {if committed {
                                view! { <A href="/tastings" attr:class="btn-primary">"View my tastings"</A> }.into_any()
                            } else if can_import {
                                view! {
                                    <label class="import-confirm"><input type="checkbox" prop:checked=move || confirmed.get() on:change=move |ev| confirmed.set(event_target_checked(&ev)) disabled=move || busy.get() />
                                        <span>"This is my export. Import these tastings into my signed-in account and add the new entries to the shared catalog."</span>
                                    </label>
                                    <button type="button" class="btn-primary" disabled=move || busy.get() || !confirmed.get() on:click=move |_| submit.run(true)>{format!("Import {} tastings", r.imported)}</button>
                                }.into_any()
                            } else { view! { <p>"There are no new rated check-ins to import."</p> }.into_any() }}
                        </section>
                    }
                })}
            </Show>
        </div>
    }
}

async fn send(token: &str, body: &str, receipt: Option<&str>) -> Result<ImportResponse, String> {
    let mut request = gloo_net::http::Request::post(if receipt.is_some() {
        "/api/imports/untappd"
    } else {
        "/api/imports/untappd/preview"
    })
    .header("Authorization", &format!("Bearer {token}"))
    .header("Content-Type", "application/json");
    if let Some(receipt) = receipt {
        request = request.header("X-Import-Preview", receipt);
    }
    let response = request.body(body).map_err(|_| "Could not prepare the upload.")?.send().await
        .map_err(|_| "Could not reach the server. The outcome is unknown; preview again to check what remains.")?;
    if !response.ok() {
        let status = response.status();
        let data = response.json::<serde_json::Value>().await.ok();
        return Err(data
            .and_then(|v| v["error"].as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("Import failed (HTTP {status}).")));
    }
    response
        .json()
        .await
        .map_err(|_| "Could not read the import result. Preview again before retrying.".into())
}
