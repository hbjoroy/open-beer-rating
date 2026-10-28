use std::sync::Arc;

use leptos::prelude::*;

/// An owned browser object URL. Dropping the last owner releases the browser's bytes.
#[derive(Debug)]
pub(crate) struct ObjectUrl(String);

impl ObjectUrl {
    pub(crate) fn from_blob(blob: &web_sys::Blob) -> Result<Self, String> {
        web_sys::Url::create_object_url_with_blob(blob)
            .map(Self)
            .map_err(|_| "Could not preview this photo. Please select it again.".to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Drop for ObjectUrl {
    fn drop(&mut self) {
        let _ = web_sys::Url::revoke_object_url(&self.0);
    }
}

/// Photos are private: fetch with authentication and render only a temporary blob URL.
#[component]
pub fn AuthenticatedPhoto(
    token: ReadSignal<Option<String>>,
    tasting_id: String,
    alt: String,
    #[prop(default = "thumb")] variant: &'static str,
    #[prop(optional)] on_expand: Option<Callback<()>>,
) -> impl IntoView {
    let (image, set_image) = signal(Option::<Arc<ObjectUrl>>::None);
    let (error, set_error) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);
    let (image_owner, set_image_owner) = signal(Option::<String>::None);
    let (retry, set_retry) = signal(0_u64);
    let generation = RwSignal::new(0_u64);
    Effect::new(move |_| {
        retry.track();
        let request_generation = generation.get_untracked().wrapping_add(1);
        generation.set(request_generation);
        // Account changes revoke the previous private photo immediately.
        set_image.set(None);
        set_error.set(None);
        set_loading.set(false);
        let owner = token.get();
        set_image_owner.set(owner.clone());
        let Some(tok) = owner else {
            return;
        };
        let id = tasting_id.clone();
        set_loading.set(true);
        leptos::task::spawn_local(async move {
            let result = fetch_photo(&tok, &id, variant).await;
            if generation.try_get_untracked() != Some(request_generation)
                || token.try_get_untracked().flatten().as_deref() != Some(&tok)
            {
                return;
            }
            match result {
                Ok(bytes) => {
                    let data = js_sys::Uint8Array::from(bytes.as_slice());
                    let parts = js_sys::Array::new();
                    parts.push(&data);
                    let options = web_sys::BlobPropertyBag::new();
                    options.set_type("image/jpeg");
                    let url =
                        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
                            .map_err(|_| "Could not display this photo.".to_string())
                            .and_then(|blob| ObjectUrl::from_blob(&blob));
                    match url {
                        Ok(url) => set_image.set(Some(Arc::new(url))),
                        Err(message) => set_error.set(Some(message)),
                    }
                }
                Err(message) => set_error.set(Some(message)),
            }
            set_loading.set(false);
        });
    });

    view! {
        <div class=if variant == "large" { "tasting-photo-large" } else { "tasting-photo-thumb" }>
            {move || loading.get().then(|| view! { <p role="status">"Loading photo..."</p> })}
            {move || error.get().map(|message| view! {
                <div class="photo-error">
                    <p role="alert">{message}</p>
                    <button type="button" on:click=move |_| set_retry.update(|value| *value = value.wrapping_add(1))>"Retry photo"</button>
                </div>
            })}
            {move || {
                // Hide an old account's image even before its cleanup effect runs.
                let current_image = if image_owner.get() == token.get() { image.get() } else { None };
                current_image.map(|url| {
                let source = url.as_str().to_string();
                if let Some(expand) = on_expand {
                    view! {
                        <button type="button" class="photo-thumbnail-button" aria-label="View larger tasting photo"
                            on:click=move |_| expand.run(())>
                            <img src=source alt=alt.clone() loading="lazy" />
                        </button>
                    }.into_any()
                } else {
                    view! { <img src=source alt=alt.clone() loading="lazy" /> }.into_any()
                }
                })
            }}
        </div>
    }
}

async fn fetch_photo(token: &str, tasting_id: &str, variant: &str) -> Result<Vec<u8>, String> {
    let response =
        gloo_net::http::Request::get(&format!("/api/tastings/{tasting_id}/photo/{variant}"))
            .header("Authorization", &format!("Bearer {token}"))
            .send()
            .await
            .map_err(|_| "Could not load photo. Please try again.".to_string())?;
    if !response.ok() {
        return Err(if response.status() == 404 {
            "This photo is no longer available."
        } else {
            "Could not load photo. Please try again."
        }
        .to_string());
    }
    response
        .binary()
        .await
        .map_err(|_| "Could not read photo. Please try again.".to_string())
}
