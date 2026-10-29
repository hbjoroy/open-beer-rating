use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};
use tower::ServiceExt;

fn state(pool: sqlx::PgPool, port: u16) -> AppState {
    AppState {
        pool,
        identity: None,
        encryption_key: [42; 32],
        jwt_secret: "test-only".into(),
        webauthn: std::sync::Arc::new(open_tappd_webauthn::WebAuthn::new(
            open_tappd_webauthn::WebAuthnConfig::new("localhost", "http://localhost:3000"),
        )),
        email_recovery: Some(RecoveryConfig {
            until: Utc::now() + chrono::Duration::days(14),
            origin: "https://app.example".into(),
            from: "Open Drinks <test@example.test>".parse().unwrap(),
            smtp: AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
                .port(port)
                .timeout(Some(std::time::Duration::from_secs(2)))
                .build(),
        }),
    }
}
async fn request(
    state: &AppState,
    path: &str,
    body: Value,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(if path == "/api/users/me/privacy" {
            "GET"
        } else {
            "POST"
        })
        .uri(path)
        .header("Origin", "https://app.example")
        .header("Content-Type", "application/json");
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let response = crate::create_router(state.clone())
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
async fn user(state: &AppState, name: &str) -> (Uuid, String, String) {
    let (status, data) = request(
        state,
        "/api/users/register",
        json!({"username":name,"email":"owner@example.test"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{data}");
    let id = Uuid::parse_str(data["user"]["id"].as_str().unwrap()).unwrap();
    (
        id,
        data["token"].as_str().unwrap().into(),
        data["recovery_key"].as_str().unwrap().into(),
    )
}
async fn enable(state: &AppState, id: Uuid) {
    sqlx::query("UPDATE users SET email_verified_at=now(),email_verification_source='test_fixture',email_recovery_allowed_until=now()+interval '14 days' WHERE id=$1").bind(id).execute(&state.pool).await.unwrap();
}
async fn queued(state: &AppState) -> Delivery {
    let bytes: Vec<u8> = sqlx::query_scalar("SELECT payload FROM email_recovery_outbox LIMIT 1")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    serde_json::from_str(&decrypt_field(&bytes, &state.encryption_key).unwrap()).unwrap()
}
// Test-only local SMTP sink, never production mail. It records the actual MIME body.
async fn smtp_sink() -> (u16, tokio::sync::oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = BufReader::new(stream);
        stream
            .get_mut()
            .write_all(b"220 test SMTP\r\n")
            .await
            .unwrap();
        let mut data = false;
        let mut body = String::new();
        let mut sender = Some(sender);
        loop {
            let mut line = String::new();
            if stream.read_line(&mut line).await.unwrap() == 0 {
                break;
            }
            let reply = if data {
                if line == ".\r\n" {
                    data = false;
                    if let Some(sender) = sender.take() {
                        let _ = sender.send(body.clone());
                    }
                    "250 queued\r\n"
                } else {
                    body.push_str(&line);
                    continue;
                }
            } else if line.starts_with("EHLO") {
                "250-test\r\n250 8BITMIME\r\n"
            } else if line.starts_with("DATA") {
                data = true;
                "354 send message\r\n"
            } else if line.starts_with("QUIT") {
                let _ = stream.get_mut().write_all(b"221 bye\r\n").await;
                break;
            } else {
                "250 OK\r\n"
            };
            stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
        }
    });
    (port, receiver)
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires isolated PostgreSQL"]
async fn mail_recovery_rotates_key_revokes_sessions_and_is_single_use(pool: sqlx::PgPool) {
    let (port, received) = smtp_sink().await;
    let state = state(pool, port);
    let (id, token, old_key) = user(&state, "email_owner").await;
    enable(&state, id).await;
    let (status, data) = request(
        &state,
        "/api/email-recovery/request",
        json!({"username":"email_owner","email":"OWNER@example.test"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(data.get("code").is_none());
    let delivery = queued(&state).await;
    assert_eq!(
        request(
            &state,
            "/api/users/login",
            json!({"username":"email_owner","recovery_key":delivery.code}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    deliver_one(&state).await.unwrap();
    let message = received.await.unwrap();
    assert!(message.contains(&delivery.code));
    assert!(message.contains("20 minutes"));
    assert!(!message.contains(&old_key));
    let (status, data) = request(
        &state,
        "/api/email-recovery/redeem",
        json!({"username":"email_owner","code":delivery.code.to_lowercase()}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{data}");
    assert!(data.get("token").is_none());
    let new_key = data["recovery_key"].as_str().unwrap();
    assert_ne!(new_key, old_key);
    assert_eq!(
        request(&state, "/api/users/me/privacy", json!({}), Some(&token))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &state,
            "/api/users/login",
            json!({"username":"email_owner","recovery_key":old_key}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, login) = request(
        &state,
        "/api/users/login",
        json!({"username":"email_owner","recovery_key":new_key}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        request(
            &state,
            "/api/users/me/privacy",
            json!({}),
            login["token"].as_str()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &state,
            "/api/email-recovery/redeem",
            json!({"username":"email_owner","code":delivery.code}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM email_recovery_outbox")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
    let export = crate::db::privacy::export_user_data(&state.pool, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        export.user.email_verification_source.as_deref(),
        Some("email_code")
    );
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires isolated PostgreSQL"]
async fn eligibility_rate_limits_expiry_and_attempts(pool: sqlx::PgPool) {
    let state = state(pool, 9);
    let (id, _, _) = user(&state, "email_owner").await;
    let body = json!({"username":"email_owner","email":"owner@example.test"});
    let (_, fresh) = request(&state, "/api/email-recovery/request", body.clone(), None).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM email_recovery_codes")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
    enable(&state, id).await;
    let (_, unknown) = request(
        &state,
        "/api/email-recovery/request",
        json!({"username":"unknown","email":"owner@example.test"}),
        None,
    )
    .await;
    let (_, wrong) = request(
        &state,
        "/api/email-recovery/request",
        json!({"username":"email_owner","email":"wrong@example.test"}),
        None,
    )
    .await;
    assert_eq!(fresh, unknown);
    assert_eq!(fresh, wrong);
    request(&state, "/api/email-recovery/request", body.clone(), None).await;
    let first = queued(&state).await.code;
    request(&state, "/api/email-recovery/request", body.clone(), None).await;
    assert_eq!(queued(&state).await.code, first);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM email_recovery_requests")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
    for _ in 0..5 {
        assert_eq!(
            request(
                &state,
                "/api/email-recovery/redeem",
                json!({"username":"email_owner","code":"wrong"}),
                None
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        request(
            &state,
            "/api/email-recovery/redeem",
            json!({"username":"email_owner","code":first}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    for _ in 0..2 {
        sqlx::query(
            "UPDATE email_recovery_requests SET requested_at=requested_at-interval '2 minutes'",
        )
        .execute(&state.pool)
        .await
        .unwrap();
        request(&state, "/api/email-recovery/request", body.clone(), None).await;
    }
    let third = queued(&state).await.code;
    sqlx::query(
        "UPDATE email_recovery_requests SET requested_at=requested_at-interval '2 minutes'",
    )
    .execute(&state.pool)
    .await
    .unwrap();
    request(&state, "/api/email-recovery/request", body.clone(), None).await;
    assert_eq!(queued(&state).await.code, third);
    sqlx::query("UPDATE email_recovery_codes SET expires_at=now()-interval '1 second'")
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &state,
            "/api/email-recovery/redeem",
            json!({"username":"email_owner","code":third}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let mut expired = state.clone();
    expired.email_recovery.as_mut().unwrap().until = Utc::now() - chrono::Duration::seconds(1);
    assert_eq!(
        request(&expired, "/api/email-recovery/request", body, None)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    crate::db::privacy::delete_user_data(&state.pool, id)
        .await
        .unwrap();
    for table in [
        "email_recovery_codes",
        "email_recovery_outbox",
        "email_recovery_requests",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&state.pool)
                .await
                .unwrap(),
            0
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires isolated PostgreSQL"]
async fn concurrent_redemption_and_failed_delivery(pool: sqlx::PgPool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let state = state(pool, port);
    let (id, _, _) = user(&state, "email_owner").await;
    enable(&state, id).await;
    request(
        &state,
        "/api/email-recovery/request",
        json!({"username":"email_owner","email":"owner@example.test"}),
        None,
    )
    .await;
    let code = queued(&state).await.code;
    assert!(deliver_one(&state).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT attempts FROM email_recovery_outbox")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
    let body = json!({"username":"email_owner","code":code});
    let (a, b) = tokio::join!(
        request(&state, "/api/email-recovery/redeem", body.clone(), None),
        request(&state, "/api/email-recovery/redeem", body, None)
    );
    let mut statuses = vec![a.0.as_u16(), b.0.as_u16()];
    statuses.sort();
    assert_eq!(statuses, vec![200, 401]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT auth_version FROM users WHERE id=$1")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "requires isolated PostgreSQL"]
async fn only_existing_email_accounts_are_bootstrapped(pool: sqlx::PgPool) {
    let migrator = sqlx::migrate!("../../migrations");
    for migration in migrator.iter().filter(|m| m.version < 20261029000001) {
        sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO users(username,recovery_key_hash,email_encrypted) VALUES('old_email','fixture',$1),('no_email','fixture',NULL)").bind(vec![42u8]).execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../migrations/20261029000001_test_email_recovery.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO users(username,recovery_key_hash,email_encrypted) VALUES('new_email','fixture',$1)").bind(vec![42u8]).execute(&pool).await.unwrap();
    let eligible:Vec<String>=sqlx::query_scalar("SELECT username FROM users WHERE email_verified_at IS NOT NULL AND email_recovery_allowed_until>now()").fetch_all(&pool).await.unwrap();
    assert_eq!(eligible, vec!["old_email"]);
}
