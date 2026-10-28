//! PostgreSQL regressions. Run with a dedicated DATABASE_URL and `cargo test -p open-tappd-api --test privacy_tastings -- --ignored`.
//! SQLx creates an isolated disposable database for each test; no shared data is deleted.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use tower::ServiceExt;
use uuid::Uuid;

fn app(pool: PgPool) -> Router {
    open_tappd_api::create_router(open_tappd_api::state::AppState {
        identity: None,
        pool,
        encryption_key: [0x42; 32],
        jwt_secret: "regression-secret".into(),
        webauthn: std::sync::Arc::new(open_tappd_webauthn::WebAuthn::new(
            open_tappd_webauthn::WebAuthnConfig {
                rp_id: "localhost".into(),
                rp_name: "Regression tests".into(),
                origin: "http://localhost:8080".into(),
            },
        )),
    })
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("Content-Type", "application/json");
    if let Some(token) = token {
        builder = builder.header("Authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn user(app: &Router, name: &str) -> (Uuid, String, String) {
    let (status, body) = request(
        app,
        "POST",
        "/api/users/register",
        None,
        json!({"username":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        Uuid::parse_str(body["user"]["id"].as_str().unwrap()).unwrap(),
        body["token"].as_str().unwrap().into(),
        body["recovery_key"].as_str().unwrap().into(),
    )
}
async fn beer(pool: &PgPool) -> Uuid {
    let brewery: Uuid = sqlx::query_scalar(
        "INSERT INTO breweries(name) VALUES ('Regression brewery') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO beers(name,brewery_id,style) VALUES ('Regression beer',$1,'IPA') RETURNING id",
    )
    .bind(brewery)
    .fetch_one(pool)
    .await
    .unwrap()
}
async fn location(app: &Router, token: &str) -> String {
    let (status, value) = request(
        app,
        "POST",
        "/api/locations",
        Some(token),
        json!({"name":"Private home","location_type":"home","metadata":{}}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{value}");
    value["id"].as_str().unwrap().into()
}
async fn session(app: &Router, token: &str, loc: &str, visibility: &str) -> Value {
    let (status, value) = request(
        app,
        "POST",
        "/api/tasting-sessions",
        Some(token),
        json!({"name":"Private gathering","location_id":loc,"visibility":visibility}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{value}");
    value
}
async fn tasting(
    app: &Router,
    token: &str,
    beer: Uuid,
    loc: Option<&str>,
    sess: Option<&str>,
    score: i32,
    time: &str,
) -> Value {
    let (status,value)=request(app,"POST","/api/tastings",Some(token),json!({"beer_id":beer,"score":score,"notes":"private tasting notes","location_id":loc,"session_id":sess,"tasted_at":time,"serving_style":"can"})).await;
    assert_eq!(status, StatusCode::CREATED, "{value}");
    value
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn feed_stays_empty_and_locations_are_owner_only(pool: PgPool) {
    let app = app(pool.clone());
    let (owner, a, _) = user(&app, "owner").await;
    let (_, b, _) = user(&app, "outsider").await;
    let loc = location(&app, &a).await;
    let beer = beer(&pool).await;
    tasting(&app, &a, beer, Some(&loc), None, 8, "2026-10-01T12:00:00Z").await;
    let encrypted: Vec<u8> =
        sqlx::query_scalar("SELECT notes_encrypted FROM tastings WHERE user_id=$1")
            .bind(owner)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(encrypted, b"private tasting notes");
    for public in [false, true] {
        sqlx::query("UPDATE user_privacy_settings SET profile_visibility = $2::profile_visibility, show_ratings=$3,show_tasting_sessions=$3 WHERE user_id=$1").bind(owner).bind(if public {"public"} else {"private"}).bind(public).execute(&pool).await.unwrap();
        for token in [None, Some(a.as_str()), Some(b.as_str())] {
            let (status, body) = request(
                &app,
                "GET",
                "/api/tastings/recent?limit=50",
                token,
                Value::Null,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body, json!([]));
        }
    }
    for path in [
        "/api/locations".to_string(),
        format!("/api/locations/{loc}"),
    ] {
        assert_eq!(
            request(&app, "GET", &path, None, Value::Null).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let (status, body) = request(&app, "GET", "/api/locations", Some(&b), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/locations/{loc}"),
            Some(&b),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/locations/{loc}"),
            Some(&a),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    for (method, path, body) in [
        (
            "POST",
            "/api/tastings".to_string(),
            json!({"beer_id":beer,"score":5,"location_id":loc}),
        ),
        (
            "POST",
            "/api/tasting-sessions".to_string(),
            json!({"name":"Foreign location","location_id":loc}),
        ),
    ] {
        assert_eq!(
            request(&app, method, &path, Some(&b), body).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let own = tasting(&app, &b, beer, None, None, 5, "2026-10-02T12:00:00Z").await;
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!("/api/tastings/{}", own["id"].as_str().unwrap()),
            Some(&b),
            json!({"score":6,"location_id":loc})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn session_membership_controls_all_visibility_levels_and_hides_private_context(pool: PgPool) {
    let app = app(pool.clone());
    let (_, a, _) = user(&app, "owner").await;
    let (_, b, _) = user(&app, "member").await;
    let (_, c, _) = user(&app, "outsider").await;
    let loc = location(&app, &a).await;
    let beer = beer(&pool).await;
    for visibility in ["private", "participants", "public"] {
        let sess = session(&app, &a, &loc, visibility).await;
        let id = sess["id"].as_str().unwrap();
        let t = tasting(
            &app,
            &a,
            beer,
            Some(&loc),
            Some(id),
            8,
            "2026-10-01T12:00:00Z",
        )
        .await;
        for suffix in ["", "/tastings"] {
            let path = format!("/api/tasting-sessions/{id}{suffix}");
            assert_eq!(
                request(&app, "GET", &path, None, Value::Null).await.0,
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                request(&app, "GET", &path, Some(&c), Value::Null).await.0,
                StatusCode::NOT_FOUND
            );
            assert_eq!(
                request(&app, "GET", &path, Some(&a), Value::Null).await.0,
                StatusCode::OK
            );
        }
        let (status, list) =
            request(&app, "GET", "/api/tasting-sessions", Some(&c), Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list, json!([]));
        let (status, joined) = request(
            &app,
            "POST",
            "/api/tasting-sessions/join",
            Some(&b),
            json!({"code":sess["join_code"]}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(joined["location_id"].is_null());
        let (_, member_sessions) =
            request(&app, "GET", "/api/tasting-sessions", Some(&b), Value::Null).await;
        assert!(
            member_sessions
                .as_array()
                .unwrap()
                .iter()
                .all(|session| session["location_id"].is_null())
        );
        let (status, detail) = request(
            &app,
            "GET",
            &format!("/api/tasting-sessions/{id}"),
            Some(&b),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(detail["location_id"].is_null());
        let (status, rows) = request(
            &app,
            "GET",
            &format!("/api/tasting-sessions/{id}/tastings"),
            Some(&b),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let row = &rows.as_array().unwrap()[0];
        assert_eq!(row["id"], t["id"]);
        for field in ["notes", "location_id", "location_name"] {
            assert!(row[field].is_null(), "leaked {field}: {row}");
        }
        let (_, rows) = request(
            &app,
            "GET",
            &format!("/api/tasting-sessions/{id}/tastings"),
            Some(&a),
            Value::Null,
        )
        .await;
        assert_eq!(rows[0]["notes"], "private tasting notes");
        assert_eq!(rows[0]["location_id"], loc);
        // Ownership must still authorize access after the owner leaves membership.
        request(
            &app,
            "POST",
            &format!("/api/tasting-sessions/{id}/leave"),
            Some(&a),
            Value::Null,
        )
        .await;
        assert_eq!(
            request(
                &app,
                "GET",
                &format!("/api/tasting-sessions/{id}"),
                Some(&a),
                Value::Null
            )
            .await
            .0,
            StatusCode::OK
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn legacy_upsert_preserves_history_context_and_aggregates_follow_edits(pool: PgPool) {
    let app = app(pool.clone());
    let (_, a, _) = user(&app, "owner").await;
    let (_, b, _) = user(&app, "other").await;
    let beer = beer(&pool).await;
    let loc = location(&app, &a).await;
    let sess = session(&app, &a, &loc, "participants").await;
    let sid = sess["id"].as_str().unwrap();
    let older = tasting(&app, &a, beer, None, None, 2, "2026-09-01T12:00:00Z").await;
    let latest = tasting(
        &app,
        &a,
        beer,
        Some(&loc),
        Some(sid),
        8,
        "2026-10-01T12:00:00Z",
    )
    .await;
    tasting(&app, &b, beer, None, None, 4, "2026-10-02T12:00:00Z").await;
    let path = format!("/api/tastings/{}", latest["id"].as_str().unwrap());
    for method in ["GET", "PUT", "DELETE"] {
        assert_eq!(
            request(&app, method, &path, Some(&b), json!({"score":1}))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(&app, method, &path, None, json!({"score":1}))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    let (status, updated) = request(
        &app,
        "POST",
        &format!("/api/beers/{beer}/ratings"),
        Some(&a),
        json!({"score":10,"notes":"replacement notes"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(updated["id"], latest["id"]);
    let (_, detail) = request(&app, "GET", &path, Some(&a), Value::Null).await;
    assert_eq!(detail["location_id"], loc);
    assert_eq!(detail["session_id"], sid);
    assert_eq!(detail["serving_style"], "can");
    assert_eq!(detail["tasted_at"], "2026-10-01T12:00:00Z");
    assert_eq!(detail["notes"], "replacement notes");
    let (_, agg) = request(
        &app,
        "GET",
        &format!("/api/beers/{beer}/ratings"),
        None,
        Value::Null,
    )
    .await;
    let (_, tastings) = request(
        &app,
        "GET",
        &format!("/api/beers/{beer}/tastings"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(agg["rating_count"], 2);
    assert_eq!(agg["average_score"], 7.0);
    assert_eq!(tastings["average_score"], agg["average_score"]);
    assert_eq!(tastings["total_tastings"], 3);
    assert_eq!(
        request(&app, "PUT", &path, Some(&a), json!({"score":6}))
            .await
            .0,
        StatusCode::OK
    );
    let (_, agg) = request(
        &app,
        "GET",
        &format!("/api/beers/{beer}/ratings"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(agg["average_score"], 5.0);
    assert_eq!(
        request(&app, "DELETE", &path, Some(&a), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, agg) = request(
        &app,
        "GET",
        &format!("/api/beers/{beer}/ratings"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(agg["average_score"], 3.0);
    let (_, ratings) = request(&app, "GET", "/api/users/me/ratings", Some(&a), Value::Null).await;
    assert_eq!(ratings[0]["id"], older["id"]);
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn concurrent_legacy_upserts_create_one_canonical_event(pool: PgPool) {
    let app = app(pool.clone());
    let (uid, _, _) = user(&app, "owner").await;
    let beer = beer(&pool).await;
    let mut tasks = tokio::task::JoinSet::new();
    for score in 0..10 {
        let pool = pool.clone();
        tasks.spawn(async move {
            open_tappd_api::db::ratings::upsert_rating(&pool, uid, beer, score, None)
                .await
                .unwrap()
        });
    }
    let mut ids = Vec::new();
    while let Some(result) = tasks.join_next().await {
        ids.push(result.unwrap());
    }
    assert!(ids.iter().all(|id| *id == ids[0]));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM tastings WHERE user_id=$1 AND beer_id=$2")
            .bind(uid)
            .bind(beer)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn export_includes_history_and_deletion_preserves_other_users_tastings(pool: PgPool) {
    let app = app(pool.clone());
    let (uid, a, key) = user(&app, "owner").await;
    let (_, b, _) = user(&app, "member").await;
    let beer = beer(&pool).await;
    let loc = location(&app, &a).await;
    let sess = session(&app, &a, &loc, "participants").await;
    let sid = sess["id"].as_str().unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/tasting-sessions/join",
            Some(&b),
            json!({"code":sess["join_code"]})
        )
        .await
        .0,
        StatusCode::OK
    );
    tasting(
        &app,
        &a,
        beer,
        Some(&loc),
        Some(sid),
        5,
        "2026-09-01T12:00:00Z",
    )
    .await;
    tasting(
        &app,
        &a,
        beer,
        Some(&loc),
        Some(sid),
        8,
        "2026-10-01T12:00:00Z",
    )
    .await;
    let other = tasting(&app, &b, beer, None, Some(sid), 6, "2026-10-02T12:00:00Z").await;
    // Represent a historical cross-owner reference; deletion must detach it safely.
    sqlx::query("UPDATE tastings SET location_id=$1 WHERE id=$2")
        .bind(Uuid::parse_str(&loc).unwrap())
        .bind(Uuid::parse_str(other["id"].as_str().unwrap()).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, export) = request(
        &app,
        "GET",
        "/api/users/me/data-export",
        Some(&a),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(export["tastings"].as_array().unwrap().len(), 2);
    assert_eq!(export["locations"].as_array().unwrap().len(), 1);
    assert_eq!(export["created_sessions"].as_array().unwrap().len(), 1);
    assert_eq!(export["session_memberships"].as_array().unwrap().len(), 1);
    assert_eq!(export["tastings"][0]["notes"], "private tasting notes");
    assert_eq!(
        request(
            &app,
            "DELETE",
            "/api/users/me",
            Some(&a),
            json!({"recovery_key":key})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "GET", "/api/users/me/ratings", Some(&a), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let own: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE id=$1")
        .bind(uid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(own, 0);
    let (_, detail) = request(
        &app,
        "GET",
        &format!("/api/tastings/{}", other["id"].as_str().unwrap()),
        Some(&b),
        Value::Null,
    )
    .await;
    assert_eq!(detail["score"], 6);
    assert_eq!(detail["notes"], "private tasting notes");
    assert!(detail["location_id"].is_null());
    assert!(detail["session_id"].is_null());
}

#[sqlx::test(migrations = false)]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn migration_preserves_unchanged_divergent_and_missing_legacy_rows(pool: PgPool) {
    for source in [
        include_str!("../../../migrations/20260508000001_initial_schema.sql"),
        include_str!("../../../migrations/20260508000002_seed_badges.sql"),
        include_str!("../../../migrations/20260509000001_add_passkeys.sql"),
        include_str!("../../../migrations/20260509000002_add_tastings.sql"),
        include_str!("../../../migrations/20260509000003_add_serving_style.sql"),
    ] {
        sqlx::raw_sql(source).execute(&pool).await.unwrap();
    }
    let uid: Uuid = sqlx::query_scalar(
        "INSERT INTO users(username,recovery_key_hash) VALUES ('legacy','fixture') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let beer = beer(&pool).await;
    let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    // Distinct users satisfy the old UNIQUE(user_id,beer_id) constraint.
    for (n, id) in ids.iter().enumerate() {
        let user = if n == 0 {
            uid
        } else {
            sqlx::query_scalar(
                "INSERT INTO users(username,recovery_key_hash) VALUES ($1,'fixture') RETURNING id",
            )
            .bind(format!("legacy{n}"))
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        sqlx::query("INSERT INTO ratings(id,user_id,beer_id,score,notes_encrypted,created_at) VALUES($1,$2,$3,8,$4,'2026-05-01T12:00:00Z')").bind(id).bind(user).bind(beer).bind(vec![1u8,2,3]).execute(&pool).await.unwrap();
        if n < 2 {
            sqlx::query("INSERT INTO tastings(id,user_id,beer_id,score,notes_encrypted,tasted_at,created_at) VALUES($1,$2,$3,$4,$5,'2026-05-01T12:00:00Z','2026-05-01T12:00:00Z')").bind(id).bind(user).bind(beer).bind(if n==1 {3} else {8}).bind(vec![1u8,2,3]).execute(&pool).await.unwrap();
        }
    }
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/20261028000001_unify_tastings.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let rows=sqlx::query("SELECT id,score,notes_encrypted,tasted_at= '2026-05-01T12:00:00Z'::timestamptz AS original_time FROM tastings").fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 4);
    for row in &rows {
        assert_eq!(row.get::<Vec<u8>, _>("notes_encrypted"), vec![1u8, 2, 3]);
        assert!(row.get::<bool, _>("original_time"));
    }
    assert_eq!(
        rows.iter()
            .find(|row| row.get::<Uuid, _>("id") == ids[1])
            .unwrap()
            .get::<i32, _>("score"),
        3
    );
    assert!(rows.iter().any(|row| row.get::<Uuid, _>("id") == ids[2]));
    let view: bool =
        sqlx::query_scalar("SELECT relkind='v' FROM pg_class WHERE oid='ratings'::regclass")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(view);
    assert!(
        sqlx::query("UPDATE ratings SET score=1")
            .execute(&pool)
            .await
            .is_err(),
        "legacy view must reject writes"
    );
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn latest_selection_is_deterministic_for_all_public_and_legacy_reads(pool: PgPool) {
    let app = app(pool.clone());
    let (uid, a, _) = user(&app, "owner").await;
    let (other, _, _) = user(&app, "other").await;
    let beer = beer(&pool).await;
    // UUID order must break the exact tasted_at + created_at tie, while tasted_at
    // must outrank a newer insertion date for a backdated tasting.
    for (id, score, tasted, created) in [
        (1u128, 2, "2026-10-01", "2026-10-02"),
        (2, 8, "2026-10-01", "2026-10-02"),
        (3, 1, "2026-09-01", "2026-10-03"),
        (4, 3, "2026-10-01", "2026-10-01"),
    ] {
        sqlx::query("INSERT INTO tastings(id,user_id,beer_id,score,tasted_at,created_at) VALUES($1,$2,$3,$4,$5::text::timestamptz,$6::text::timestamptz)").bind(Uuid::from_u128(id)).bind(uid).bind(beer).bind(score).bind(tasted).bind(created).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO tastings(user_id,beer_id,score) VALUES($1,$2,4)")
        .bind(other)
        .bind(beer)
        .execute(&pool)
        .await
        .unwrap();
    let (_, ratings) = request(&app, "GET", "/api/users/me/ratings", Some(&a), Value::Null).await;
    assert_eq!(ratings.as_array().unwrap().len(), 1);
    assert_eq!(ratings[0]["id"], Uuid::from_u128(2).to_string());
    assert_eq!(ratings[0]["score"], 8);
    let score: i32 =
        sqlx::query_scalar("SELECT score FROM ratings WHERE user_id=$1 AND beer_id=$2")
            .bind(uid)
            .bind(beer)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(score, 8);
    let (_, agg) = request(
        &app,
        "GET",
        &format!("/api/beers/{beer}/ratings"),
        None,
        Value::Null,
    )
    .await;
    let (_, events) = request(
        &app,
        "GET",
        &format!("/api/beers/{beer}/tastings"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(agg["average_score"], 6.0);
    assert_eq!(agg["rating_count"], 2);
    assert_eq!(events["average_score"], 6.0);
    assert_eq!(events["unique_tasters"], 2);
    assert_eq!(events["total_tastings"], 5);
    let id = open_tappd_api::db::ratings::upsert_rating(&pool, uid, beer, 9, None)
        .await
        .unwrap();
    assert_eq!(id, Uuid::from_u128(2));
}

#[sqlx::test(migrations = false)]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn migration_collision_aborts_without_mutating_either_history(pool: PgPool) {
    for source in [
        include_str!("../../../migrations/20260508000001_initial_schema.sql"),
        include_str!("../../../migrations/20260508000002_seed_badges.sql"),
        include_str!("../../../migrations/20260509000001_add_passkeys.sql"),
        include_str!("../../../migrations/20260509000002_add_tastings.sql"),
        include_str!("../../../migrations/20260509000003_add_serving_style.sql"),
    ] {
        sqlx::raw_sql(source).execute(&pool).await.unwrap();
    }
    let a: Uuid = sqlx::query_scalar(
        "INSERT INTO users(username,recovery_key_hash) VALUES ('owner','fixture') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let b: Uuid = sqlx::query_scalar(
        "INSERT INTO users(username,recovery_key_hash) VALUES ('other','fixture') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let beer = beer(&pool).await;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO ratings(id,user_id,beer_id,score) VALUES($1,$2,$3,8)")
        .bind(id)
        .bind(a)
        .bind(beer)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tastings(id,user_id,beer_id,score) VALUES($1,$2,$3,3)")
        .bind(id)
        .bind(b)
        .bind(beer)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let result = sqlx::raw_sql(include_str!(
        "../../../migrations/20261028000001_unify_tastings.sql"
    ))
    .execute(&mut *tx)
    .await;
    assert!(result.is_err());
    tx.rollback().await.unwrap();
    let rating: (Uuid, i32) = sqlx::query_as("SELECT user_id,score FROM ratings WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rating, (a, 8));
    let tasting: (Uuid, i32) = sqlx::query_as("SELECT user_id,score FROM tastings WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tasting, (b, 3));
    let table: bool =
        sqlx::query_scalar("SELECT relkind='r' FROM pg_class WHERE oid='ratings'::regclass")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(table);
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn historical_foreign_locations_are_masked_from_own_history_and_exports(pool: PgPool) {
    let app = app(pool.clone());
    let (uid, a, _) = user(&app, "owner").await;
    let (_, b, _) = user(&app, "location_owner").await;
    let loc = location(&app, &b).await;
    let loc_id = Uuid::parse_str(&loc).unwrap();
    let beer = beer(&pool).await;
    // These references predate ownership validation and cannot be created by
    // current HTTP endpoints. Reading old records must still protect locations.
    let sid: Uuid = sqlx::query_scalar("INSERT INTO tasting_sessions(name,created_by,location_id,join_code) VALUES ('Historical session',$1,$2,'LEGACY') RETURNING id")
        .bind(uid).bind(loc_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO session_participants(session_id,user_id) VALUES($1,$2)")
        .bind(sid)
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    let tid: Uuid = sqlx::query_scalar("INSERT INTO tastings(user_id,beer_id,score,location_id,session_id) VALUES($1,$2,7,$3,$4) RETURNING id")
        .bind(uid).bind(beer).bind(loc_id).bind(sid).fetch_one(&pool).await.unwrap();
    let (status, detail) = request(
        &app,
        "GET",
        &format!("/api/tastings/{tid}"),
        Some(&a),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(detail["location_id"].is_null());
    assert!(detail["location_name"].is_null());
    for path in [
        "/api/tastings".into(),
        format!("/api/tasting-sessions/{sid}/tastings"),
    ] {
        let (status, rows) = request(&app, "GET", &path, Some(&a), Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert!(rows[0]["location_id"].is_null());
        assert!(rows[0]["location_name"].is_null());
    }
    let (status, detail) = request(
        &app,
        "GET",
        &format!("/api/tasting-sessions/{sid}"),
        Some(&a),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(detail["location_id"].is_null());
    let (status, rows) = request(&app, "GET", "/api/tasting-sessions", Some(&a), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert!(rows[0]["location_id"].is_null());
    let (status, joined) = request(
        &app,
        "POST",
        "/api/tasting-sessions/join",
        Some(&a),
        json!({"code":"LEGACY"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(joined["location_id"].is_null());
    let (status, export) = request(
        &app,
        "GET",
        "/api/users/me/data-export",
        Some(&a),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(export["tastings"].as_array().unwrap().len(), 1);
    assert!(export["tastings"][0]["location_id"].is_null());
    assert!(export["tastings"][0]["location_name"].is_null());
    assert!(export["created_sessions"][0]["location_id"].is_null());
    assert_eq!(export["locations"], json!([]));
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn export_has_no_history_cap_and_contains_only_passkey_metadata(pool: PgPool) {
    let app = app(pool.clone());
    let (uid, token, _) = user(&app, "owner").await;
    let (other, _, _) = user(&app, "other").await;
    let beer = beer(&pool).await;
    sqlx::query("INSERT INTO tastings(user_id,beer_id,score,tasted_at) SELECT $1,$2,7,'2026-01-01'::timestamptz + n * interval '1 second' FROM generate_series(1,10001) AS n")
        .bind(uid).bind(beer).execute(&pool).await.unwrap();
    let passkey: Uuid = sqlx::query_scalar("INSERT INTO user_passkeys(user_id,credential_id,public_key_cbor,name,transports,counter) VALUES($1,$2,$3,'My phone','internal',42) RETURNING id")
        .bind(uid).bind(vec![1u8,2,3]).bind(vec![4u8,5,6]).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_passkeys(user_id,credential_id,public_key_cbor,name) VALUES($1,$2,$3,'Other user secret key')")
        .bind(other).bind(vec![7u8,8,9]).bind(vec![10u8,11,12]).execute(&pool).await.unwrap();
    let (status, export) = request(
        &app,
        "GET",
        "/api/users/me/data-export",
        Some(&token),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(export["tastings"].as_array().unwrap().len(), 10001);
    assert_eq!(export["ratings"].as_array().unwrap().len(), 1);
    let passkeys = export["passkeys"].as_array().unwrap();
    assert_eq!(passkeys.len(), 1);
    assert_eq!(passkeys[0]["id"], passkey.to_string());
    assert_eq!(passkeys[0]["name"], "My phone");
    assert!(passkeys[0]["created_at"].is_string());
    let mut keys: Vec<_> = passkeys[0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["created_at", "id", "name"]);
    for forbidden in [
        "credential_id",
        "public_key_cbor",
        "recovery_key_hash",
        "password_hash",
    ] {
        assert!(!export.to_string().contains(forbidden));
    }
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn history_filter_is_scoped_to_owner_and_catalog_searches_brewery(pool: PgPool) {
    let app = app(pool.clone());
    let (_, owner, _) = user(&app, "filterowner").await;
    let (_, other, _) = user(&app, "filterother").await;
    let first = beer(&pool).await;
    let second = beer(&pool).await;
    tasting(&app, &owner, first, None, None, 7, "2026-10-01T12:00:00Z").await;
    tasting(&app, &owner, second, None, None, 5, "2026-10-02T12:00:00Z").await;
    tasting(&app, &other, first, None, None, 2, "2026-10-03T12:00:00Z").await;
    let (status, rows) = request(
        &app,
        "GET",
        &format!("/api/tastings?beer_id={first}"),
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["score"], 7);
    let (_, all) = request(&app, "GET", "/api/tastings", Some(&owner), Value::Null).await;
    assert_eq!(all.as_array().unwrap().len(), 2);
    sqlx::query("UPDATE breweries SET name='Unique brewery search' WHERE id=(SELECT brewery_id FROM beers WHERE id=$1)")
        .bind(first).execute(&pool).await.unwrap();
    let (status, found) = request(
        &app,
        "GET",
        "/api/beers?search=Unique%20brewery",
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(found[0]["id"], first.to_string());
}

async fn photo_request(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    bytes: Vec<u8>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("Content-Type", "image/png");
    if let Some(token) = token {
        builder = builder.header("Authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(bytes)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn tasting_photos_are_owner_only_resized_and_cascade_deleted(pool: PgPool) {
    use image::{GenericImageView, ImageFormat};
    let app = app(pool.clone());
    let (_, owner, recovery) = user(&app, "photo_owner").await;
    let (_, other, _) = user(&app, "photo_other").await;
    let beer = beer(&pool).await;
    let t = tasting(&app, &owner, beer, None, None, 7, "2026-10-28T12:00:00Z").await;
    let id = t["id"].as_str().unwrap();
    let path = format!("/api/tastings/{id}/photo");
    let mut source = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1800, 1200)
        .write_to(&mut source, ImageFormat::Png)
        .unwrap();
    let bytes = source.into_inner();
    assert_eq!(
        photo_request(&app, "PUT", &path, None, bytes.clone())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        photo_request(&app, "PUT", &path, Some(&other), bytes.clone())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(
        photo_request(&app, "PUT", &path, Some(&owner), bytes.clone())
            .await
            .0
            .is_success()
    );
    for (variant, dimensions) in [("thumb", (480, 320)), ("large", (1440, 960))] {
        let url = format!("{path}/{variant}");
        assert_eq!(
            photo_request(&app, "GET", &url, None, vec![]).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            photo_request(&app, "GET", &url, Some(&other), vec![])
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        let (status, headers, encoded) =
            photo_request(&app, "GET", &url, Some(&owner), vec![]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], "image/jpeg");
        assert!(
            headers["cache-control"]
                .to_str()
                .unwrap()
                .contains("no-store")
        );
        assert_eq!(
            image::load_from_memory(&encoded).unwrap().dimensions(),
            dimensions
        );
    }
    let (_, own) = request(
        &app,
        "GET",
        &format!("/api/tastings/{id}"),
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(own["has_photo"], true);
    let (status, export) = request(
        &app,
        "GET",
        "/api/users/me/data-export",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let photo = &export["tastings"][0]["photo"];
    assert_eq!(photo["content_type"], "image/jpeg");
    use base64::Engine;
    let exported = base64::engine::general_purpose::STANDARD
        .decode(photo["large_base64"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        image::load_from_memory(&exported).unwrap().dimensions(),
        (1440, 960)
    );
    let (_, other_export) = request(
        &app,
        "GET",
        "/api/users/me/data-export",
        Some(&other),
        Value::Null,
    )
    .await;
    assert!(!other_export.to_string().contains(id));
    // An invalid replacement must not erase the previously stored image.
    assert_eq!(
        photo_request(&app, "PUT", &path, Some(&owner), b"not an image".to_vec())
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        photo_request(&app, "GET", &format!("{path}/thumb"), Some(&owner), vec![])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        photo_request(
            &app,
            "PUT",
            &path,
            Some(&owner),
            vec![0; 12 * 1024 * 1024 + 1]
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/api/tastings/{id}"),
            Some(&owner),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tasting_photos")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let t = tasting(&app, &owner, beer, None, None, 6, "2026-10-28T13:00:00Z").await;
    let path = format!("/api/tastings/{}/photo", t["id"].as_str().unwrap());
    assert!(
        photo_request(&app, "PUT", &path, Some(&owner), bytes)
            .await
            .0
            .is_success()
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            "/api/users/me",
            Some(&owner),
            json!({"recovery_key": recovery})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tasting_photos")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn heic_upload_produces_private_jpeg_variants(pool: PgPool) {
    use image::GenericImageView;
    let app = app(pool.clone());
    let (_, token, _) = user(&app, "heic_owner").await;
    let beer_id = beer(&pool).await;
    let tasting = tasting(&app, &token, beer_id, None, None, 8, "2026-10-28T15:00:00Z").await;
    let id = tasting["id"].as_str().unwrap();
    let path = format!("/api/tastings/{id}/photo");
    // The server sniffs the container rather than trusting the declared content type.
    let source = include_bytes!("fixtures/heic/checker-1024.heic").to_vec();
    assert_eq!(
        photo_request(&app, "PUT", &path, Some(&token), source)
            .await
            .0,
        StatusCode::OK
    );
    for (variant, edge) in [("thumb", 320), ("large", 960)] {
        let (status, headers, bytes) = photo_request(
            &app,
            "GET",
            &format!("{path}/{variant}"),
            Some(&token),
            vec![],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], "image/jpeg");
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().dimensions(),
            (edge, edge)
        );
        assert!(!bytes.windows(4).any(|part| part == b"Exif"));
    }
}
