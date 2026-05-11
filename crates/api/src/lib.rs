pub mod auth;
pub mod db;
pub mod errors;
pub mod routes;
pub mod state;

use axum::{Router, routing::{get, post, put, delete}};

pub fn create_router(state: state::AppState) -> Router {
    let api = Router::new()
        .route("/health", get(routes::health::health_check))
        // Users
        .route("/api/users/register", post(routes::users::register))
        .route("/api/users/login", post(routes::users::login))
        // Passkeys
        .route("/api/passkeys/register/start", post(routes::passkeys::register_start))
        .route("/api/passkeys/register/finish", post(routes::passkeys::register_finish))
        .route("/api/passkeys/auth/start", post(routes::passkeys::auth_start))
        .route("/api/passkeys/auth/finish", post(routes::passkeys::auth_finish))
        .route("/api/passkeys", get(routes::passkeys::list_passkeys))
        .route("/api/passkeys/{id}", delete(routes::passkeys::delete_passkey))
        // Breweries
        .route("/api/breweries", post(routes::breweries::create_brewery))
        .route("/api/breweries", get(routes::breweries::list_breweries))
        .route("/api/breweries/{id}", get(routes::breweries::get_brewery))
        // Beers
        .route("/api/beers", post(routes::beers::create_beer))
        .route("/api/beers", get(routes::beers::list_beers))
        .route("/api/beers/{id}", get(routes::beers::get_beer))
        // Ratings (deprecated — use /api/tastings)
        .route("/api/beers/{id}/ratings", post(routes::ratings::rate_beer))
        .route("/api/beers/{id}/ratings", get(routes::ratings::get_beer_ratings))
        .route("/api/users/me/ratings", get(routes::ratings::get_my_ratings))
        // Tastings (replaces ratings)
        .route("/api/tastings", post(routes::tastings::create_tasting))
        .route("/api/tastings", get(routes::tastings::list_my_tastings))
        .route("/api/tastings/recent", get(routes::tastings::get_recent_tastings))
        .route("/api/tastings/{id}", get(routes::tastings::get_tasting))
        .route("/api/tastings/{id}", put(routes::tastings::update_tasting))
        .route("/api/tastings/{id}", delete(routes::tastings::delete_tasting))
        .route("/api/beers/{id}/tastings", get(routes::tastings::get_beer_tastings))
        // Tasting Sessions
        .route("/api/tasting-sessions", post(routes::tasting_sessions::create_session))
        .route("/api/tasting-sessions", get(routes::tasting_sessions::list_sessions))
        .route("/api/tasting-sessions/join", post(routes::tasting_sessions::join_session_by_code))
        .route("/api/tasting-sessions/{id}", get(routes::tasting_sessions::get_session))
        .route("/api/tasting-sessions/{id}/join", post(routes::tasting_sessions::join_session))
        .route("/api/tasting-sessions/{id}/leave", post(routes::tasting_sessions::leave_session))
        .route("/api/tasting-sessions/{id}/end", post(routes::tasting_sessions::end_session))
        .route("/api/tasting-sessions/{id}/tastings", get(routes::tasting_sessions::get_session_tastings))
        // Locations
        .route("/api/locations", post(routes::locations::create_location))
        .route("/api/locations", get(routes::locations::list_locations))
        .route("/api/locations/{id}", get(routes::locations::get_location))
        .route("/api/locations/{id}", put(routes::locations::update_location))
        .route("/api/locations/{id}", delete(routes::locations::delete_location))
        // Badges
        .route("/api/users/me/badges", get(routes::badges::get_my_badges))
        // Privacy & Data Sovereignty
        .route("/api/users/me/privacy", get(routes::privacy::get_privacy_settings))
        .route("/api/users/me/privacy", put(routes::privacy::update_privacy_settings))
        .route("/api/users/me/data-export", get(routes::privacy::export_data))
        .route("/api/users/me", delete(routes::privacy::delete_account))
        .with_state(state);

    // Serve static frontend assets if the directory exists (production Docker build)
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "./static".into());
    if std::path::Path::new(&static_dir).exists() {
        tracing::info!("Serving static frontend from {}", static_dir);
        api.fallback_service(
            tower_http::services::ServeDir::new(&static_dir)
                .fallback(tower_http::services::ServeFile::new(
                    format!("{}/index.html", static_dir),
                )),
        )
    } else {
        api
    }
}
