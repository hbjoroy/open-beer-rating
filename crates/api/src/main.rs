use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

use open_tappd_api::state::AppState;

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

    // Migrations are applied automatically on startup.
    // If you pre-applied them via scripts/migrate-db.ps1, SQLx will
    // detect they're already done and skip them.
    match sqlx::migrate!("../../migrations").run(&pool).await {
        Ok(()) => tracing::info!("Migrations applied successfully"),
        Err(e) => {
            tracing::warn!("Migration warning: {e}");
            tracing::info!("If tables already exist, this is safe to ignore.");
        }
    }

    let state = AppState {
        pool,
        encryption_key,
        jwt_secret,
    };

    let app = open_tappd_api::create_router(state);

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
