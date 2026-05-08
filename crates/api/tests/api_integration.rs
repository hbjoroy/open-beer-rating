//! Integration tests for the Open Tappd API.
//!
//! Tests marked with `#[ignore]` require a running PostgreSQL database.
//! Run them with: `cargo test -- --ignored`
//!
//! Set these environment variables before running:
//! - DATABASE_URL=postgres://opentappd:opentappd@localhost:5432/opentappd_test
//! - ENCRYPTION_KEY=<base64-encoded 32-byte key>
//! - JWT_SECRET=test-secret

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

/// Helper to create a test app with a real database connection.
async fn test_app() -> axum::Router {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://opentappd:opentappd@localhost:5432/opentappd_test".into());

    let encryption_key = open_tappd_domain::crypto::parse_encryption_key(
        &std::env::var("ENCRYPTION_KEY").unwrap_or_else(|_| {
            // Generate a deterministic test key
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode([0x42u8; 32])
        }),
    )
    .expect("valid encryption key");

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "test-secret".into());

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let state = open_tappd_api::state::AppState {
        pool,
        encryption_key,
        jwt_secret,
    };

    open_tappd_api::create_router(state)
}

/// Helper to read the response body as JSON.
async fn body_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Helper to register a user and return the response.
async fn register_user(app: &axum::Router, username: &str, password: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/api/users/register")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "username": username,
                "password": password
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = body_json(resp.into_body()).await;
    (status, body)
}

/// Helper to login and return the JWT token.
async fn login_user(app: &axum::Router, username: &str, password: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/users/login")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "username": username,
                "password": password
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp.into_body()).await;
    body["token"].as_str().unwrap().to_string()
}

// ──────────────────────────────────────────────
// Health check (no DB required — but needs router)
// ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn health_check_returns_ok() {
    let app = test_app().await;

    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp.into_body()).await;
    assert_eq!(body["status"], "ok");
}

// ──────────────────────────────────────────────
// User Registration
// ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn register_user_minimal() {
    let app = test_app().await;
    let username = format!("testuser_{}", uuid::Uuid::new_v4().simple());

    let (status, body) = register_user(&app, &username, "securepassword123").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["username"], username);
    assert!(body["id"].is_string());
    // Password hash should NOT be in the response
    assert!(body.get("password_hash").is_none());
}

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn register_user_with_email() {
    let app = test_app().await;
    let username = format!("emailuser_{}", uuid::Uuid::new_v4().simple());

    let req = Request::builder()
        .method("POST")
        .uri("/api/users/register")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "username": username,
                "password": "securepassword123",
                "email": "test@example.com"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn register_duplicate_username_returns_409() {
    let app = test_app().await;
    let username = format!("dupuser_{}", uuid::Uuid::new_v4().simple());

    let (status, _) = register_user(&app, &username, "password12345678").await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = register_user(&app, &username, "password12345678").await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn register_short_password_returns_422() {
    let app = test_app().await;
    let username = format!("shortpw_{}", uuid::Uuid::new_v4().simple());

    let (status, _) = register_user(&app, &username, "short").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ──────────────────────────────────────────────
// User Authentication
// ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn login_valid_credentials() {
    let app = test_app().await;
    let username = format!("loginuser_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;

    let token = login_user(&app, &username, "securepassword123").await;
    assert!(!token.is_empty());
    // JWT has 3 parts separated by dots
    assert_eq!(token.split('.').count(), 3);
}

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn login_wrong_password_returns_401() {
    let app = test_app().await;
    let username = format!("wrongpw_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/users/login")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "username": username,
                "password": "wrongpassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ──────────────────────────────────────────────
// Brewery CRUD
// ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn create_and_list_breweries() {
    let app = test_app().await;
    let username = format!("brewer_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;
    let token = login_user(&app, &username, "securepassword123").await;

    // Create a brewery
    let req = Request::builder()
        .method("POST")
        .uri("/api/breweries")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({
                "name": "Test Brewery",
                "country": "Belgium",
                "city": "Brussels"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let brewery = body_json(resp.into_body()).await;
    assert_eq!(brewery["name"], "Test Brewery");

    // List breweries
    let req = Request::builder()
        .uri("/api/breweries")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let breweries = body_json(resp.into_body()).await;
    assert!(breweries.as_array().unwrap().len() >= 1);
}

// ──────────────────────────────────────────────
// Beer CRUD
// ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn create_and_get_beer() {
    let app = test_app().await;
    let username = format!("beerlover_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;
    let token = login_user(&app, &username, "securepassword123").await;

    // Create brewery first
    let req = Request::builder()
        .method("POST")
        .uri("/api/breweries")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({
                "name": "Beer Test Brewery",
                "country": "Germany"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let brewery = body_json(resp.into_body()).await;
    let brewery_id = brewery["id"].as_str().unwrap();

    // Create a beer
    let req = Request::builder()
        .method("POST")
        .uri("/api/beers")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({
                "name": "Test IPA",
                "brewery_id": brewery_id,
                "style": "IPA",
                "abv": 6.5
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let beer = body_json(resp.into_body()).await;
    let beer_id = beer["id"].as_str().unwrap();

    // Get the beer
    let req = Request::builder()
        .uri(format!("/api/beers/{beer_id}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let fetched = body_json(resp.into_body()).await;
    assert_eq!(fetched["name"], "Test IPA");
}

// ──────────────────────────────────────────────
// Ratings (privacy-first: aggregate only)
// ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn rate_beer_and_check_aggregate() {
    let app = test_app().await;
    let username = format!("rater_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;
    let token = login_user(&app, &username, "securepassword123").await;

    // Create brewery + beer
    let req = Request::builder()
        .method("POST")
        .uri("/api/breweries")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(json!({"name": "Rating Brewery", "country": "UK"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let brewery = body_json(resp.into_body()).await;
    let brewery_id = brewery["id"].as_str().unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/api/beers")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({"name": "Rating Test Stout", "brewery_id": brewery_id, "style": "Stout", "abv": 5.0}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let beer = body_json(resp.into_body()).await;
    let beer_id = beer["id"].as_str().unwrap();

    // Rate the beer
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/beers/{beer_id}/ratings"))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(json!({"score": 8, "notes": "Great stout!"}).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Get aggregate ratings (public, no auth needed)
    let req = Request::builder()
        .uri(format!("/api/beers/{beer_id}/ratings"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let agg = body_json(resp.into_body()).await;
    assert_eq!(agg["count"], 1);
    // Average should be 8.0
    assert!(agg["average"].as_f64().unwrap() > 7.9);

    // Get my own ratings (private, needs auth)
    let req = Request::builder()
        .uri("/api/users/me/ratings")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let my_ratings = body_json(resp.into_body()).await;
    let ratings_arr = my_ratings.as_array().unwrap();
    assert_eq!(ratings_arr.len(), 1);
    assert_eq!(ratings_arr[0]["score"], 8);
}

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn rating_score_out_of_range_rejected() {
    let app = test_app().await;
    let username = format!("badscore_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;
    let token = login_user(&app, &username, "securepassword123").await;

    // Create brewery + beer
    let req = Request::builder()
        .method("POST")
        .uri("/api/breweries")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(json!({"name": "Score Brewery", "country": "NL"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let brewery = body_json(resp.into_body()).await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/beers")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({"name": "Score Test Beer", "brewery_id": brewery["id"], "style": "Lager"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let beer = body_json(resp.into_body()).await;

    // Try to rate with score 11 (out of range, max is 10)
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/beers/{}/ratings", beer["id"].as_str().unwrap()))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(json!({"score": 11}).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Try negative score
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/beers/{}/ratings", beer["id"].as_str().unwrap()))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(json!({"score": -1}).to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ──────────────────────────────────────────────
// Privacy & Data Sovereignty
// ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn privacy_settings_default_to_private() {
    let app = test_app().await;
    let username = format!("privuser_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;
    let token = login_user(&app, &username, "securepassword123").await;

    let req = Request::builder()
        .uri("/api/users/me/privacy")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let settings = body_json(resp.into_body()).await;
    // Default: everything private
    assert_eq!(settings["profile_visibility"], "private");
    assert_eq!(settings["show_ratings"], false);
    assert_eq!(settings["show_badges"], false);
    assert_eq!(settings["show_stats"], false);
}

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn update_privacy_settings() {
    let app = test_app().await;
    let username = format!("updpriv_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;
    let token = login_user(&app, &username, "securepassword123").await;

    let req = Request::builder()
        .method("PUT")
        .uri("/api/users/me/privacy")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({
                "profile_visibility": "public",
                "show_badges": true,
                "show_ratings": false,
                "show_stats": false
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify the update
    let req = Request::builder()
        .uri("/api/users/me/privacy")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let settings = body_json(resp.into_body()).await;
    assert_eq!(settings["profile_visibility"], "public");
    assert_eq!(settings["show_badges"], true);
    assert_eq!(settings["show_ratings"], false);
}

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn data_export_includes_all_user_data() {
    let app = test_app().await;
    let username = format!("export_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;
    let token = login_user(&app, &username, "securepassword123").await;

    let req = Request::builder()
        .uri("/api/users/me/data-export")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let export = body_json(resp.into_body()).await;
    assert!(export.get("user").is_some());
    assert!(export.get("privacy_settings").is_some());
}

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn account_deletion_removes_all_data() {
    let app = test_app().await;
    let username = format!("deluser_{}", uuid::Uuid::new_v4().simple());
    register_user(&app, &username, "securepassword123").await;
    let token = login_user(&app, &username, "securepassword123").await;

    // Delete account
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/users/me")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(json!({"password": "securepassword123"}).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Try to login again — should fail
    let req = Request::builder()
        .method("POST")
        .uri("/api/users/login")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "username": username,
                "password": "securepassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ──────────────────────────────────────────────
// Auth enforcement
// ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL"]
async fn unauthenticated_requests_rejected() {
    let app = test_app().await;

    // Creating a brewery without auth should fail
    let req = Request::builder()
        .method("POST")
        .uri("/api/breweries")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"name": "No Auth Brewery"}).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Rating a beer without auth should fail
    let req = Request::builder()
        .method("POST")
        .uri("/api/beers/00000000-0000-0000-0000-000000000000/ratings")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"score": 5}).to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
