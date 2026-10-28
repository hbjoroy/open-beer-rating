use leptos::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct BreweryOption {
    id: String,
    name: String,
}

#[component]
pub fn AddBeerPage(token: ReadSignal<Option<String>>) -> impl IntoView {
    let (beer_name, set_beer_name) = signal(String::new());
    let (style, set_style) = signal(String::new());
    let (abv, set_abv) = signal(String::new());
    let (description, set_description) = signal(String::new());

    let (creating_brewery, set_creating_brewery) = signal(false);
    let (selected_brewery_id, set_selected_brewery_id) = signal(String::new());
    let (new_brewery_name, set_new_brewery_name) = signal(String::new());
    let (new_brewery_country, set_new_brewery_country) = signal(String::new());
    let (new_brewery_city, set_new_brewery_city) = signal(String::new());

    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<(String, String)>::None);
    let (loading, set_loading) = signal(false);

    let breweries = leptos::prelude::LocalResource::new(|| fetch_breweries());

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if loading.get() {
            return;
        }
        let tok = match token.get() {
            Some(t) => t,
            None => {
                set_error.set(Some("Please log in".to_string()));
                return;
            }
        };

        let is_new_brewery = creating_brewery.get();
        let brewery_id = selected_brewery_id.get();
        let bname = new_brewery_name.get().trim().to_string();
        let bcountry = new_brewery_country.get();
        let bcity = new_brewery_city.get();
        let name = beer_name.get().trim().to_string();
        let style_val = style.get();
        let abv_val = abv.get();
        let desc_val = description.get();
        let abv_parsed =
            match validate_beer_fields(&name, &abv_val, is_new_brewery, &brewery_id, &bname) {
                Ok(abv) => abv,
                Err(e) => {
                    set_error.set(Some(e));
                    return;
                }
            };
        set_loading.set(true);
        set_error.set(None);
        set_success.set(None);

        leptos::task::spawn_local(async move {
            let bid = if is_new_brewery {
                if bname.is_empty() {
                    set_error.set(Some("Brewery name is required".to_string()));
                    set_loading.set(false);
                    return;
                }
                match create_brewery_api(
                    &tok,
                    &bname,
                    if bcountry.is_empty() {
                        None
                    } else {
                        Some(&bcountry)
                    },
                    if bcity.is_empty() { None } else { Some(&bcity) },
                )
                .await
                {
                    Ok(id) => {
                        if token.get_untracked().as_deref() != Some(&tok) {
                            return;
                        }
                        set_selected_brewery_id.set(id.clone());
                        set_creating_brewery.set(false);
                        breweries.refetch();
                        id
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                        set_loading.set(false);
                        return;
                    }
                }
            } else {
                if brewery_id.is_empty() {
                    set_error.set(Some("Please select a brewery".to_string()));
                    set_loading.set(false);
                    return;
                }
                brewery_id
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
                if desc_val.is_empty() {
                    None
                } else {
                    Some(&desc_val)
                },
            )
            .await
            {
                Ok(created) => {
                    if token.get_untracked().as_deref() != Some(&tok) {
                        return;
                    }
                    set_success.set(Some(created));
                    set_beer_name.set(String::new());
                    set_style.set(String::new());
                    set_abv.set(String::new());
                    set_description.set(String::new());
                    set_loading.set(false);
                }
                Err(e) => {
                    set_error.set(Some(e));
                    set_loading.set(false);
                }
            }
        });
    };

    view! {
        <div class="page add-beer-page">
            <h2>"Add a Beer 🍺"</h2>

            {move || {
                if token.get().is_none() {
                    Some(view! { <p class="login-prompt"><a href="/login?next=%2Fbeers%2Fnew">"Log in to add a beer"</a></p> })
                } else {
                    None
                }
            }}

            {move || success.get().map(|(id, name)| view! {
                <div class="success" role="status">
                    <p>{format!("Added {name}.")}</p>
                    <a href=format!("/beers/{id}")>"View beer"</a>
                    " "
                    <a href=format!("/tastings/new?beer={id}")>"Record a tasting"</a>
                </div>
            })}

            <form on:submit=on_submit style:display=move || if token.get().is_none() { "none" } else { "block" }>
                <fieldset class="fieldset">
                    <legend>"Brewery"</legend>

                    <div class="form-group">
                        <label class="checkbox-label">
                            <input
                                type="checkbox"
                                prop:checked=move || creating_brewery.get()
                                on:change=move |_| set_creating_brewery.update(|v| *v = !*v)
                            />
                            " Create new brewery"
                        </label>
                    </div>

                    <div style:display=move || if creating_brewery.get() { "block" } else { "none" }>
                        <div class="form-group">
                            <label for="add-new_brewery_name">"Brewery Name"</label>
                            <input
                                type="text"
                                id="add-new_brewery_name"
                                prop:value=move || new_brewery_name.get()
                                on:input=move |ev| set_new_brewery_name.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="form-row">
                            <div class="form-group">
                                <label for="add-new_brewery_country">"Country"</label>
                                <input
                                    type="text"
                                    id="add-new_brewery_country"
                                prop:value=move || new_brewery_country.get()
                                    on:input=move |ev| set_new_brewery_country.set(event_target_value(&ev))
                                    placeholder="e.g. Norway"
                                />
                            </div>
                            <div class="form-group">
                                <label for="add-new_brewery_city">"City"</label>
                                <input
                                    type="text"
                                    id="add-new_brewery_city"
                                prop:value=move || new_brewery_city.get()
                                    on:input=move |ev| set_new_brewery_city.set(event_target_value(&ev))
                                    placeholder="e.g. Oslo"
                                />
                            </div>
                        </div>
                    </div>

                    <div style:display=move || if creating_brewery.get() { "none" } else { "block" }>
                        <div class="form-group">
                            <label for="add-selected_brewery_id">"Select Brewery"</label>
                            <Suspense fallback=move || view! { <p>"Loading breweries..."</p> }>
                                {move || {
                                    breweries.get().map(|result| {
                                        match &*result {
                                            Ok(list) => {
                                                if list.is_empty() {
                                                    view! { <p class="hint">"No breweries yet — check \"Create new brewery\" above"</p> }.into_any()
                                                } else {
                                                    view! {
                                                        <select
                                                            id="add-selected_brewery_id"
                                prop:value=move || selected_brewery_id.get()
                                                            on:change=move |ev| set_selected_brewery_id.set(event_target_value(&ev))
                                                        >
                                                            <option value="">"— Choose a brewery —"</option>
                                                            {list.iter().map(|b| {
                                                                let id = b.id.clone();
                                                                let name = b.name.clone();
                                                                view! { <option value=id>{name}</option> }
                                                            }).collect::<Vec<_>>()}
                                                        </select>
                                                    }.into_any()
                                                }
                                            }
                                            Err(e) => view! { <p class="error">{format!("Error: {e}")}</p> }.into_any(),
                                        }
                                    })
                                }}
                            </Suspense>
                        </div>
                    </div>
                </fieldset>

                <div class="form-group">
                    <label for="add-beer_name">"Beer Name"</label>
                    <input
                        type="text"
                        id="add-beer_name"
                                prop:value=move || beer_name.get()
                        on:input=move |ev| set_beer_name.set(event_target_value(&ev))
                        placeholder="e.g. Punk IPA"
                        required
                    />
                </div>

                <div class="form-row">
                    <div class="form-group">
                        <label for="add-style">"Style (optional)"</label>
                        <input
                            type="text"
                            id="add-style"
                                prop:value=move || style.get()
                            on:input=move |ev| set_style.set(event_target_value(&ev))
                            placeholder="e.g. IPA, Stout, Lager"
                        />
                    </div>
                    <div class="form-group">
                        <label for="add-abv">"ABV % (optional)"</label>
                        <input
                            type="number"
                            step="0.1"
                            min="0"
                            max="100"
                            id="add-abv"
                                prop:value=move || abv.get()
                            on:input=move |ev| set_abv.set(event_target_value(&ev))
                            placeholder="5.6"
                        />
                    </div>
                </div>

                <div class="form-group">
                    <label for="add-description">"Description (optional)"</label>
                    <textarea
                        id="add-description"
                                prop:value=move || description.get()
                        on:input=move |ev| set_description.set(event_target_value(&ev))
                        rows="3"
                        placeholder="Tasting notes, appearance, aroma..."
                    ></textarea>
                </div>

                {move || error.get().map(|e| view! { <p class="error" role="alert">{e}</p> })}

                <button type="submit" disabled=move || loading.get()>
                    {move || if loading.get() { "Adding..." } else { "Add Beer 🍺" }}
                </button>
            </form>
        </div>
    }
}

async fn fetch_breweries() -> Result<Vec<BreweryOption>, String> {
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

async fn create_brewery_api(
    token: &str,
    name: &str,
    country: Option<&str>,
    city: Option<&str>,
) -> Result<String, String> {
    let mut body = serde_json::json!({ "name": name });
    if let Some(c) = country {
        body["country"] = serde_json::Value::String(c.to_string());
    }
    if let Some(c) = city {
        body["city"] = serde_json::Value::String(c.to_string());
    }

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
        let data: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::json!({"error": "Failed to create brewery"}));
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
    description: Option<&str>,
) -> Result<(String, String), String> {
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
    if let Some(d) = description {
        body["description"] = serde_json::Value::String(d.to_string());
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
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
        let id = data["id"]
            .as_str()
            .ok_or_else(|| "No beer ID in response".to_string())?;
        Ok((
            id.to_string(),
            data["name"].as_str().unwrap_or("Beer").to_string(),
        ))
    } else {
        let data: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::json!({"error": "Failed to create beer"}));
        Err(data["error"]
            .as_str()
            .unwrap_or("Failed to create beer")
            .to_string())
    }
}

/// Validate before either brewery or beer is created, so invalid forms do not create orphans.
pub(super) fn validate_beer_fields(
    name: &str,
    abv: &str,
    new_brewery: bool,
    brewery_id: &str,
    brewery_name: &str,
) -> Result<Option<f64>, String> {
    if name.trim().is_empty() {
        return Err("Beer name is required".into());
    }
    if name.trim().len() > 200 {
        return Err("Beer name must be at most 200 characters".into());
    }
    if new_brewery && brewery_name.trim().is_empty() {
        return Err("Brewery name is required".into());
    }
    if new_brewery && brewery_name.trim().len() > 200 {
        return Err("Brewery name must be at most 200 characters".into());
    }
    if !new_brewery && brewery_id.trim().is_empty() {
        return Err("Please select a brewery".into());
    }
    if abv.trim().is_empty() {
        return Ok(None);
    }
    let value: f64 = abv
        .trim()
        .parse()
        .map_err(|_| "ABV must be a number between 0 and 100".to_string())?;
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err("ABV must be a number between 0 and 100".into());
    }
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::validate_beer_fields;

    #[test]
    fn validates_complete_form_before_creation() {
        assert!(validate_beer_fields("  ", "5", true, "", "Brewery").is_err());
        assert!(validate_beer_fields("Beer", "5", true, "", "  ").is_err());
        assert!(validate_beer_fields(&"x".repeat(201), "5", true, "", "Brewery").is_err());
        assert!(validate_beer_fields("Beer", "5", true, "", &"x".repeat(201)).is_err());
        assert!(validate_beer_fields("Beer", "5", false, "", "").is_err());
        for invalid in ["abc", "NaN", "inf", "-0.1", "100.1"] {
            assert!(
                validate_beer_fields("Beer", invalid, true, "", "Brewery").is_err(),
                "{invalid}"
            );
        }
        assert_eq!(
            validate_beer_fields(" Beer ", " 5.6 ", false, "id", ""),
            Ok(Some(5.6))
        );
        assert_eq!(
            validate_beer_fields("Beer", "", true, "", "Brewery"),
            Ok(None)
        );
        assert_eq!(
            validate_beer_fields("Beer", "0", true, "", "Brewery"),
            Ok(Some(0.0))
        );
        assert_eq!(
            validate_beer_fields("Beer", "100", true, "", "Brewery"),
            Ok(Some(100.0))
        );
    }
}
