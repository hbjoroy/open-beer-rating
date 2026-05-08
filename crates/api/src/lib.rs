pub mod auth;
pub mod db;
pub mod errors;
pub mod routes;
pub mod state;

use axum::{Router, routing::{get, post, put, delete}};

pub fn create_router(state: state::AppState) -> Router {
    Router::new()
        .route("/health", get(routes::health::health_check))
        // Users
        .route("/api/users/register", post(routes::users::register))
        .route("/api/users/login", post(routes::users::login))
        // Breweries
        .route("/api/breweries", post(routes::breweries::create_brewery))
        .route("/api/breweries", get(routes::breweries::list_breweries))
        .route("/api/breweries/{id}", get(routes::breweries::get_brewery))
        // Beers
        .route("/api/beers", post(routes::beers::create_beer))
        .route("/api/beers", get(routes::beers::list_beers))
        .route("/api/beers/{id}", get(routes::beers::get_beer))
        // Ratings
        .route("/api/beers/{id}/ratings", post(routes::ratings::rate_beer))
        .route("/api/beers/{id}/ratings", get(routes::ratings::get_beer_ratings))
        .route("/api/users/me/ratings", get(routes::ratings::get_my_ratings))
        // Badges
        .route("/api/users/me/badges", get(routes::badges::get_my_badges))
        // Privacy & Data Sovereignty
        .route("/api/users/me/privacy", get(routes::privacy::get_privacy_settings))
        .route("/api/users/me/privacy", put(routes::privacy::update_privacy_settings))
        .route("/api/users/me/data-export", get(routes::privacy::export_data))
        .route("/api/users/me", delete(routes::privacy::delete_account))
        .with_state(state)
}
