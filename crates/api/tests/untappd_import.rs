//! Isolated PostgreSQL tests; the personal export is never a committed fixture.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn app(pool: PgPool) -> Router {
    open_tappd_api::create_router(open_tappd_api::state::AppState {
        pool,
        identity: None,
        email_recovery: None,
        encryption_key: [0x42; 32],
        jwt_secret: "import-tests".into(),
        webauthn: std::sync::Arc::new(open_tappd_webauthn::WebAuthn::new(
            open_tappd_webauthn::WebAuthnConfig {
                rp_id: "localhost".into(),
                rp_name: "Tests".into(),
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
    receipt: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("Content-Type", "application/json");
    if let Some(token) = token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    if let Some(receipt) = receipt {
        req = req.header("X-Import-Preview", receipt);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn user(pool: &PgPool, name: &str) -> (Uuid, String) {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users(username,recovery_key_hash) VALUES($1,'unused') RETURNING id",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_privacy_settings(user_id) VALUES($1)")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    (
        id,
        open_tappd_api::auth::jwt::create_versioned_token(id, name, "import-tests", 0).unwrap(),
    )
}
fn fixture() -> Value {
    json!([{"checkin_id":1001,"bid":2001,"brewery_id":3001,"beer_name":"Test IPA",
        "brewery_name":"Test Brewery","brewery_country":"Norway","brewery_city":"Bergen",
        "beer_type":"IPA","beer_abv":6.5,"beer_ibu":42,"rating_score":3.75,
        "comment":"My private note","created_at":"2024-01-02T20:30:00+02:00","serving_type":"Can",
        "venue_id":4001,"venue_name":"Test Venue","venue_city":"Bergen","venue_country":"Norway",
        "venue_lat":60.39,"venue_lng":5.32,"photo_url":"https://example.invalid/photo.jpg",
        "flavor_profiles":["Citrus"],"tagged_friends":["must not be stored"]}])
}
async fn preview(app: &Router, token: &str, body: &Value) -> Value {
    let (status, result) = request(
        app,
        "POST",
        "/api/imports/untappd/preview",
        Some(token),
        None,
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    result
}
async fn import(app: &Router, token: &str, body: &Value) -> Value {
    let p = preview(app, token, body).await;
    let (status, result) = request(
        app,
        "POST",
        "/api/imports/untappd",
        Some(token),
        p["preview_token"].as_str(),
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    result
}
async fn counts(pool: &PgPool) -> (i64, i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM breweries),(SELECT count(*) FROM beers),(SELECT count(*) FROM locations),(SELECT count(*) FROM tastings)")
        .fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn three_users_share_catalog_and_reimports_preserve_history(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let app = app(pool.clone());
    let data = fixture();
    let (uid, a) = user(&pool, "alice").await;
    let (_, b) = user(&pool, "bravo").await;
    let (_, c) = user(&pool, "charlie").await;
    let p = preview(&app, &a, &data).await;
    assert_eq!(p["report"]["imported"], 1);
    assert_eq!(p["report"]["rounded_scores"], 1);
    assert_eq!(counts(&pool).await, (0, 0, 0, 0));
    for token in [&a, &b, &c] {
        import(&app, token, &data).await;
    }
    assert_eq!(counts(&pool).await, (1, 1, 1, 3));
    sqlx::query("UPDATE tastings SET score=9 WHERE user_id=$1")
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    let repeated = import(&app, &a, &data).await;
    assert_eq!(repeated["report"]["already_imported"], 1);
    assert_eq!(repeated["report"]["imported"], 0);
    let score: i32 = sqlx::query_scalar("SELECT score FROM tastings WHERE user_id=$1")
        .bind(uid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(score, 9);
    let (_, export) = request(
        &app,
        "GET",
        "/api/users/me/data-export",
        Some(&a),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(export["tastings"][0]["notes"], "My private note");
    assert_eq!(
        export["tastings"][0]["untappd_source"]["rating_score"],
        3.75
    );
    assert_eq!(export["tastings"][0]["tasted_at"], "2024-01-02T18:30:00Z");
    assert_eq!(export["tastings"][0]["serving_style"], "can");
    assert!(!export.to_string().contains("must not be stored"));
    let encrypted: Vec<u8> =
        sqlx::query_scalar("SELECT source_encrypted FROM untappd_checkins WHERE user_id=$1")
            .bind(uid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!String::from_utf8_lossy(&encrypted).contains("photo.jpg"));
    let (_, history) = request(&app, "GET", "/api/tastings", Some(&b), None, Value::Null).await;
    let loc = history[0]["location_id"].as_str().unwrap();
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/locations/{loc}"),
            Some(&c),
            None,
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    let foreign = &export["tastings"][0]["id"].as_str().unwrap();
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/tastings/{foreign}"),
            Some(&b),
            None,
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    open_tappd_api::db::privacy::delete_user_data(&pool, uid)
        .await
        .unwrap();
    assert_eq!(counts(&pool).await, (1, 1, 1, 2));
    let sources: i64 = sqlx::query_scalar("SELECT count(*) FROM untappd_checkins")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sources, 2);
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn links_preexisting_catalog_without_overwriting_it(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let app = app(pool.clone());
    let (_, token) = user(&pool, "alice").await;
    let brewery:Uuid=sqlx::query_scalar("INSERT INTO breweries(name,country,city) VALUES('  TEST   BREWERY ','Norway','Bergen') RETURNING id").fetch_one(&pool).await.unwrap();
    let beer:Uuid=sqlx::query_scalar("INSERT INTO beers(name,brewery_id,style,abv,description) VALUES('Test IPA',$1,'IPA',6.5,'Keep this') RETURNING id").bind(brewery).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO locations(name,location_type,metadata) VALUES('Test Venue','bar','{\"city\":\"Bergen\",\"country\":\"Norway\"}')").execute(&pool).await.unwrap();
    let r = import(&app, &token, &fixture()).await;
    assert_eq!(r["report"]["breweries"]["reused"], 1);
    assert_eq!(r["report"]["beers"]["reused"], 1);
    assert_eq!(r["report"]["locations"]["reused"], 1);
    assert_eq!(counts(&pool).await, (1, 1, 1, 1));
    let (linked, description): (Uuid, String) = sqlx::query_as(
        "SELECT t.beer_id,b.description FROM tastings t JOIN beers b ON b.id=t.beer_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, beer);
    assert_eq!(description, "Keep this");
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn validation_and_ambiguous_matches_roll_back_everything(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let app = app(pool.clone());
    let (_, token) = user(&pool, "alice").await;
    for (field, value) in [
        ("rating_score", json!(6)),
        ("created_at", json!("bad")),
        ("bid", json!(0)),
        ("beer_name", json!("")),
        ("venue_id", json!(null)),
    ] {
        let mut data = fixture();
        data[0][field] = value;
        assert_eq!(
            request(
                &app,
                "POST",
                "/api/imports/untappd/preview",
                Some(&token),
                None,
                data
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(counts(&pool).await, (0, 0, 0, 0));
    }
    sqlx::query("INSERT INTO breweries(name) VALUES('Test Brewery'),('Test Brewery')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/imports/untappd/preview",
            Some(&token),
            None,
            fixture()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(counts(&pool).await, (2, 0, 0, 0));
    sqlx::query("DELETE FROM breweries")
        .execute(&pool)
        .await
        .unwrap();
    let mut data = fixture();
    let mut duplicate = data[0].clone();
    duplicate["comment"] = json!("conflicting data");
    data.as_array_mut().unwrap().push(duplicate);
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/imports/untappd/preview",
            Some(&token),
            None,
            data
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(counts(&pool).await, (0, 0, 0, 0));
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn preview_is_bound_to_account_file_and_catalog(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let app = app(pool.clone());
    let (_, a) = user(&pool, "alice").await;
    let (_, b) = user(&pool, "bravo").await;
    let data = fixture();
    let p = preview(&app, &a, &data).await;
    let receipt = p["preview_token"].as_str();
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/imports/untappd/preview",
            None,
            None,
            data.clone()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/imports/untappd",
            Some(&a),
            None,
            data.clone()
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/imports/untappd",
            Some(&b),
            receipt,
            data.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut changed = data.clone();
    changed[0]["rating_score"] = json!(4.5);
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/imports/untappd",
            Some(&a),
            receipt,
            changed
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(counts(&pool).await, (0, 0, 0, 0));
    import(&app, &b, &data).await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/imports/untappd",
            Some(&a),
            receipt,
            data.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(counts(&pool).await, (1, 1, 1, 1));
    import(&app, &a, &data).await;
    assert_eq!(counts(&pool).await, (1, 1, 1, 2));
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn duplicate_rows_unrated_and_zero_have_distinct_meanings(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let app = app(pool.clone());
    let (_, token) = user(&pool, "alice").await;
    let mut data = fixture();
    let row = data[0].clone();
    data.as_array_mut().unwrap().push(row.clone());
    let mut unrated = row.clone();
    unrated["checkin_id"] = json!(1002);
    unrated["rating_score"] = Value::Null;
    let mut zero = row;
    zero["checkin_id"] = json!(1003);
    zero["rating_score"] = json!(0);
    data.as_array_mut().unwrap().extend([unrated, zero]);
    let result = import(&app, &token, &data).await;
    assert_eq!(result["report"]["imported"], 2);
    assert_eq!(result["report"]["duplicate_rows"], 1);
    assert_eq!(result["report"]["unrated_checkins"], json!([1002]));
    let min: i32 = sqlx::query_scalar("SELECT min(score) FROM tastings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(min, 0);
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn concurrent_imports_never_create_duplicates(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let app = app(pool.clone());
    let (_, a) = user(&pool, "alice").await;
    let (_, b) = user(&pool, "bravo").await;
    let data = fixture();
    let pa = preview(&app, &a, &data).await;
    let pb = preview(&app, &b, &data).await;
    let (ra, rb) = tokio::join!(
        request(
            &app,
            "POST",
            "/api/imports/untappd",
            Some(&a),
            pa["preview_token"].as_str(),
            data.clone()
        ),
        request(
            &app,
            "POST",
            "/api/imports/untappd",
            Some(&b),
            pb["preview_token"].as_str(),
            data.clone()
        )
    );
    assert!([ra.0, rb.0].contains(&StatusCode::OK));
    assert!([ra.0, rb.0].contains(&StatusCode::CONFLICT));
    import(&app, &a, &data).await;
    import(&app, &b, &data).await;
    assert_eq!(counts(&pool).await, (1, 1, 1, 2));
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn source_ids_override_renames_but_do_not_merge_distinct_products(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let app = app(pool.clone());
    let (_, a) = user(&pool, "alice").await;
    let (_, b) = user(&pool, "bravo").await;
    import(&app, &a, &fixture()).await;
    let mut renamed = fixture();
    renamed[0]["beer_name"] = json!("New name");
    renamed[0]["brewery_name"] = json!("Renamed brewery");
    renamed[0]["venue_name"] = json!("New venue name");
    import(&app, &b, &renamed).await;
    assert_eq!(counts(&pool).await, (1, 1, 1, 2));
    let mut distinct = fixture();
    distinct[0]["bid"] = json!(2002);
    distinct[0]["checkin_id"] = json!(1002);
    import(&app, &a, &distinct).await;
    assert_eq!(counts(&pool).await, (1, 2, 1, 3));
    let mut conflict = fixture();
    conflict[0]["checkin_id"] = json!(1003);
    conflict[0]["brewery_id"] = json!(3002);
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/imports/untappd/preview",
            Some(&a),
            None,
            conflict
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(counts(&pool).await, (1, 2, 1, 3));
}

#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn shared_location_creation_and_search_reuse_existing_venue(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let app = app(pool.clone());
    let (_, a) = user(&pool, "alice").await;
    let (_, b) = user(&pool, "bravo").await;
    import(&app, &a, &fixture()).await;
    let (_,created)=request(&app,"POST","/api/locations",Some(&b),None,json!({"name":" TEST  VENUE ","location_type":"bar","metadata":{"city":"Bergen","country":"Norway"}})).await;
    assert!(created["id"].is_string());
    assert_eq!(counts(&pool).await, (1, 1, 1, 1));
    let (status, found) = request(
        &app,
        "GET",
        "/api/locations?search=Bergen&limit=100",
        Some(&b),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(created["id"], found[0]["id"]);
}

#[sqlx::test(migrations = false)]
#[ignore = "requires dedicated PostgreSQL test server"]
async fn migration_merges_catalog_repoints_history_and_removes_legacy_homes(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let mut tx = pool.begin().await.unwrap();
    let migrations = sqlx::migrate!("../../migrations");
    for migration in migrations.iter().filter(|m| m.version < 20261029000002) {
        sqlx::raw_sql(&migration.sql)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    let (uid, _) = user(&pool, "alice").await;
    let breweries:Vec<Uuid>=sqlx::query_scalar("INSERT INTO breweries(name,country) VALUES('Duplicate','Norway'),(' DUPLICATE ','Norway') RETURNING id").fetch_all(&pool).await.unwrap();
    let mut beers = Vec::new();
    for brewery in breweries {
        let id:Uuid=sqlx::query_scalar("INSERT INTO beers(name,brewery_id,style,abv) VALUES('Same beer',$1,'IPA',6) RETURNING id").bind(brewery).fetch_one(&pool).await.unwrap();
        beers.push(id);
    }
    let locations:Vec<Uuid>=sqlx::query_scalar("INSERT INTO locations(name,location_type,created_by) VALUES('Venue','bar',$1),(' VENUE ','bar',$1),('Home','home',$1) RETURNING id").bind(uid).fetch_all(&pool).await.unwrap();
    for (i, loc) in locations.iter().enumerate() {
        sqlx::query("INSERT INTO tastings(user_id,beer_id,score,location_id) VALUES($1,$2,$3,$4)")
            .bind(uid)
            .bind(beers[i % 2])
            .bind(i as i32 + 5)
            .bind(loc)
            .execute(&pool)
            .await
            .unwrap();
    }
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/20261029000002_shared_catalog_import.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(counts(&pool).await, (1, 1, 1, 3));
    let (beers,locations,missing,sum):(i64,i64,i64,i64)=sqlx::query_as("SELECT count(DISTINCT beer_id),count(DISTINCT location_id),count(*) FILTER(WHERE location_id IS NULL),sum(score) FROM tastings").fetch_one(&pool).await.unwrap();
    assert_eq!((beers, locations, missing, sum), (1, 1, 1, 18));
    open_tappd_api::db::privacy::delete_user_data(&pool, uid)
        .await
        .unwrap();
    assert_eq!(counts(&pool).await, (1, 1, 1, 0));
}

// Opt-in local rehearsal; CI uses only synthetic fixtures above.
#[cfg(feature = "external-untappd-fixture-tests")]
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires local personal fixture and dedicated PostgreSQL server"]
async fn personal_export_three_users(pool: PgPool) {
    let _guard = TEST_LOCK.lock().await;
    let path = std::env::var("UNTAPPD_FIXTURE_PATH").expect("UNTAPPD_FIXTURE_PATH");
    let data: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let app = app(pool.clone());
    let (_, a) = user(&pool, "alice").await;
    let (_, b) = user(&pool, "bravo").await;
    let (_, c) = user(&pool, "charlie").await;
    let r = import(&app, &a, &data).await;
    assert_eq!(r["report"]["rows"], 4240);
    assert_eq!(r["report"]["imported"], 4234);
    let first = counts(&pool).await;
    for token in [&b, &c] {
        import(&app, token, &data).await;
    }
    let last = counts(&pool).await;
    assert_eq!((first.0, first.1, first.2), (last.0, last.1, last.2));
    assert_eq!(last.3, 4234 * 3);
    let repeat = import(&app, &a, &data).await;
    assert_eq!(repeat["report"]["already_imported"], 4234);
    println!(
        "Rehearsal: {} breweries, {} beers, {} shared venues; {} tastings across 3 users; repeated import adds 0.",
        last.0, last.1, last.2, last.3
    );
}
