use axum::{Router, routing::{get, post}};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

mod auth;
mod db;
mod errors;
mod routes;
mod state;

use state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let encryption_key = open_tappd_domain::crypto::parse_encryption_key(
        &std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set"),
    )
    .expect("ENCRYPTION_KEY must be a valid 32-byte base64-encoded key");

    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Migrations applied successfully");

    let state = AppState {
        pool,
        encryption_key,
        jwt_secret,
    };

    let app = Router::new()
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
        .with_state(state);

    let host = std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("API_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .expect("API_PORT must be a valid port number");

    let addr = SocketAddr::new(host.parse().unwrap(), port);
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
