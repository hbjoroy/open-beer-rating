use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use openidconnect::{
    Audience, EmptyAdditionalClaims, PrivateSigningKey, StandardClaims, SubjectIdentifier, core::*,
};
use serde_json::Value;
use tower::ServiceExt;

#[test]
fn signed_identity_requires_exact_issuer_audience_nonce_expiry_and_key() {
    let key = CoreRsaPrivateSigningKey::from_pem(
        include_str!("../../tests/fixtures/oidc-test-key.pem"),
        None,
    )
    .unwrap();
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(ISSUER.into()).unwrap(),
        vec![Audience::new("test-client".into())],
        chrono::Utc::now() + chrono::Duration::minutes(5),
        chrono::Utc::now(),
        StandardClaims::new(SubjectIdentifier::new("stable-subject".into())),
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new("nonce".into())));
    for case in [
        "valid",
        "issuer",
        "audience",
        "nonce",
        "expired",
        "signature",
    ] {
        let mut c = claims.clone();
        match case {
            "issuer" => c = c.set_issuer(IssuerUrl::new(SPROYT_ISSUER.into()).unwrap()),
            "audience" => c = c.set_audiences(vec![Audience::new("sproyt".into())]),
            "nonce" => c = c.set_nonce(Some(Nonce::new("wrong".into()))),
            "expired" => c = c.set_expiration(chrono::Utc::now() - chrono::Duration::minutes(5)),
            _ => {}
        }
        let token = CoreIdToken::new(
            c,
            &key,
            CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
            None,
            None,
        )
        .unwrap();
        let keys = if case == "signature" {
            vec![]
        } else {
            vec![key.as_verification_key()]
        };
        let verifier = CoreIdTokenVerifier::new_public_client(
            ClientId::new("test-client".into()),
            IssuerUrl::new(ISSUER.into()).unwrap(),
            CoreJsonWebKeySet::new(keys),
        );
        assert_eq!(
            verified_claims(&token, verifier, &Nonce::new("nonce".into())).is_ok(),
            case == "valid",
            "{case}"
        );
    }
}

fn state(pool: sqlx::PgPool) -> AppState {
    AppState {
        email_recovery: None,
        pool,
        encryption_key: [42; 32],
        jwt_secret: "test-only".into(),
        webauthn: std::sync::Arc::new(open_tappd_webauthn::WebAuthn::new(
            open_tappd_webauthn::WebAuthnConfig::new("localhost", "http://localhost:3000"),
        )),
        identity: Some(IdentityConfig {
            client_id: "test-client".into(),
            client_secret: "test-only".into(),
            origin: "https://app.example".into(),
            http: reqwest::Client::new(),
        }),
    }
}
async fn user(state: &AppState, name: &str) -> Uuid {
    let id = sqlx::query_scalar(
        "INSERT INTO users(username,recovery_key_hash) VALUES($1,'fixture') RETURNING id",
    )
    .bind(name)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_privacy_settings(user_id) VALUES($1)")
        .bind(id)
        .execute(&state.pool)
        .await
        .unwrap();
    id
}
async fn pending_flow(state: &AppState, user: Option<Uuid>, subject: &str) -> String {
    let key = random();
    let payload = encode(
        &Payload {
            auth_version: 0,
            nonce: String::new(),
            verifier: String::new(),
            subject: Some(subject.into()),
            display_name: Some("same-name".into()),
        },
        state,
    )
    .unwrap();
    sqlx::query("INSERT INTO identity_flows(state_hash,browser_hash,user_id,stage,payload,expires_at) VALUES($1,$1,$2,'confirm',$3,now()+interval '5 minutes')")
        .bind(hash(&key)).bind(user).bind(payload).execute(&state.pool).await.unwrap();
    key
}
async fn confirm(state: &AppState, key: &str, origin: &str) -> (StatusCode, Value) {
    let app = crate::create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/identity/complete")
                .header("Origin", origin)
                .header("Cookie", format!("{COOKIE}={key}"))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires isolated PostgreSQL"]
async fn explicit_link_login_replay_conflicts_and_cascade(pool: sqlx::PgPool) {
    let state = state(pool);
    let alice = user(&state, "alice").await;
    let bob = user(&state, "bob").await;
    let key = pending_flow(&state, Some(alice), "subject-a").await;
    assert_eq!(
        confirm(&state, &key, "https://evil.example").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        confirm(&state, &random(), "https://app.example").await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = confirm(&state, &key, "https://app.example").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        crate::auth::jwt::verify_token(body["token"].as_str().unwrap(), "test-only")
            .unwrap()
            .sub,
        alice
    );
    assert_eq!(
        confirm(&state, &key, "https://app.example").await.0,
        StatusCode::UNAUTHORIZED
    );
    // Neither duplicate external identities nor changing an existing account's identity merges accounts.
    for (owner, subject) in [(bob, "subject-a"), (alice, "subject-b")] {
        let key = pending_flow(&state, Some(owner), subject).await;
        assert_eq!(
            confirm(&state, &key, "https://app.example").await.0,
            StatusCode::CONFLICT
        );
    }
    let key = pending_flow(&state, None, "subject-a").await;
    let (status, body) = confirm(&state, &key, "https://app.example").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        crate::auth::jwt::verify_token(body["token"].as_str().unwrap(), "test-only")
            .unwrap()
            .sub,
        alice
    );
    let key = pending_flow(&state, None, "unknown-subject").await;
    assert_eq!(
        confirm(&state, &key, "https://app.example").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        2
    );
    let export = crate::db::privacy::export_user_data(&state.pool, alice)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        export.identity.unwrap().sproyt_user_id,
        sproyt_id("subject-a")
    );
    let key = pending_flow(&state, Some(alice), "subject-a").await;
    sqlx::query("UPDATE users SET auth_version=auth_version+1 WHERE id=$1")
        .bind(alice)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        confirm(&state, &key, "https://app.example").await.0,
        StatusCode::UNAUTHORIZED
    );
    let key = pending_flow(&state, Some(alice), "subject-a").await;
    crate::db::privacy::delete_user_data(&state.pool, alice)
        .await
        .unwrap();
    assert_eq!(
        confirm(&state, &key, "https://app.example").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM external_identities")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires isolated PostgreSQL"]
async fn expiry_and_concurrent_confirmation(pool: sqlx::PgPool) {
    let state = state(pool);
    let alice = user(&state, "alice").await;
    let key = pending_flow(&state, Some(alice), "subject").await;
    sqlx::query("UPDATE identity_flows SET expires_at=now()-interval '1 second'")
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        confirm(&state, &key, "https://app.example").await.0,
        StatusCode::UNAUTHORIZED
    );
    let key = pending_flow(&state, Some(alice), "subject").await;
    let (a, b) = tokio::join!(
        confirm(&state, &key, "https://app.example"),
        confirm(&state, &key, "https://app.example")
    );
    let mut codes = vec![a.0.as_u16(), b.0.as_u16()];
    codes.sort();
    assert_eq!(codes, vec![200, 401]);
    let key = pending_flow(&state, Some(alice), "subject").await;
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, format!("{COOKIE}={key}").parse().unwrap());
    headers.insert(header::ORIGIN, "https://app.example".parse().unwrap());
    let response = cancel(State(state.clone()), headers, Json(serde_json::json!({})))
        .await
        .unwrap();
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        confirm(&state, &key, "https://app.example").await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires isolated PostgreSQL"]
async fn callback_is_browser_bound_and_consumed_even_on_provider_denial(pool: sqlx::PgPool) {
    let state = state(pool);
    let csrf = random();
    let browser = random();
    sqlx::query("INSERT INTO identity_flows(state_hash,browser_hash,stage,payload,expires_at) VALUES($1,$2,'authorize',$3,now()+interval '5 minutes')")
        .bind(hash(&csrf)).bind(hash(&browser)).bind(vec![0u8]).execute(&state.pool).await.unwrap();
    let query = || Callback {
        state: csrf.clone(),
        code: None,
        error: Some("access_denied".into()),
    };
    assert!(
        callback_inner(&state, &HeaderMap::new(), query())
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity_flows")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("{COOKIE}={browser}").parse().unwrap(),
    );
    assert!(callback_inner(&state, &headers, query()).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity_flows")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
    assert!(callback_inner(&state, &headers, query()).await.is_err());
}
