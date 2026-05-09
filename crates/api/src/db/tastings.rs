use chrono::{DateTime, Utc};
use open_tappd_domain::models::tasting::ServingStyle;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct TastingRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub beer_id: Uuid,
    pub score: i32,
    pub serving_style: Option<ServingStyle>,
    pub notes_encrypted: Option<Vec<u8>>,
    pub location_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub tasted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct TastingWithDetailsRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub beer_id: Uuid,
    pub beer_name: String,
    pub brewery_name: String,
    pub score: i32,
    pub serving_style: Option<ServingStyle>,
    pub notes_encrypted: Option<Vec<u8>>,
    pub location_id: Option<Uuid>,
    pub location_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub session_name: Option<String>,
    pub tasted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Public recent tasting (for the feed) — includes username
#[derive(Debug, FromRow)]
pub struct RecentTastingRow {
    pub id: Uuid,
    pub username: String,
    pub beer_id: Uuid,
    pub beer_name: String,
    pub beer_style: Option<String>,
    pub brewery_name: String,
    pub score: i32,
    pub serving_style: Option<ServingStyle>,
    pub location_name: Option<String>,
    pub session_name: Option<String>,
    pub tasted_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct TastingAggregateRow {
    pub average_score: Option<f64>,
    pub tasting_count: Option<i64>,
}

pub async fn create_tasting(
    pool: &PgPool,
    user_id: Uuid,
    beer_id: Uuid,
    score: i32,
    serving_style: Option<ServingStyle>,
    notes_encrypted: Option<&[u8]>,
    location_id: Option<Uuid>,
    session_id: Option<Uuid>,
    tasted_at: DateTime<Utc>,
) -> Result<TastingRow, sqlx::Error> {
    sqlx::query_as::<_, TastingRow>(
        r#"
        INSERT INTO tastings (user_id, beer_id, score, serving_style, notes_encrypted, location_id, session_id, tasted_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, user_id, beer_id, score, serving_style, notes_encrypted, location_id, session_id,
                  tasted_at, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(beer_id)
    .bind(score)
    .bind(serving_style)
    .bind(notes_encrypted)
    .bind(location_id)
    .bind(session_id)
    .bind(tasted_at)
    .fetch_one(pool)
    .await
}

pub async fn get_tasting(pool: &PgPool, id: Uuid) -> Result<Option<TastingRow>, sqlx::Error> {
    sqlx::query_as::<_, TastingRow>(
        r#"
        SELECT id, user_id, beer_id, score, serving_style, notes_encrypted, location_id, session_id,
               tasted_at, created_at, updated_at
        FROM tastings WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn update_tasting(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    score: i32,
    serving_style: Option<ServingStyle>,
    notes_encrypted: Option<&[u8]>,
    location_id: Option<Uuid>,
) -> Result<Option<TastingRow>, sqlx::Error> {
    sqlx::query_as::<_, TastingRow>(
        r#"
        UPDATE tastings
        SET score = $3, serving_style = $4, notes_encrypted = $5, location_id = $6, updated_at = now()
        WHERE id = $1 AND user_id = $2
        RETURNING id, user_id, beer_id, score, serving_style, notes_encrypted, location_id, session_id,
                  tasted_at, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(score)
    .bind(serving_style)
    .bind(notes_encrypted)
    .bind(location_id)
    .fetch_optional(pool)
    .await
}

pub async fn delete_tasting(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM tastings WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn get_user_tastings(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<TastingWithDetailsRow>, sqlx::Error> {
    sqlx::query_as::<_, TastingWithDetailsRow>(
        r#"
        SELECT t.id, t.user_id, t.beer_id, b.name as beer_name, br.name as brewery_name,
               t.score, t.serving_style, t.notes_encrypted,
               t.location_id, l.name as location_name,
               t.session_id, ts.name as session_name,
               t.tasted_at, t.created_at
        FROM tastings t
        JOIN beers b ON b.id = t.beer_id
        JOIN breweries br ON br.id = b.brewery_id
        LEFT JOIN locations l ON l.id = t.location_id
        LEFT JOIN tasting_sessions ts ON ts.id = t.session_id
        WHERE t.user_id = $1
        ORDER BY t.tasted_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Get tastings for a specific beer.
/// Uses latest-per-user aggregation to prevent score stuffing.
pub async fn get_beer_aggregate(
    pool: &PgPool,
    beer_id: Uuid,
) -> Result<TastingAggregateRow, sqlx::Error> {
    sqlx::query_as::<_, TastingAggregateRow>(
        r#"
        SELECT
            AVG(score::float) as average_score,
            COUNT(*) as tasting_count
        FROM (
            SELECT DISTINCT ON (user_id) score
            FROM tastings
            WHERE beer_id = $1
            ORDER BY user_id, tasted_at DESC
        ) latest
        "#,
    )
    .bind(beer_id)
    .fetch_one(pool)
    .await
}

/// Total tasting count for a beer (all tastings, not deduplicated)
pub async fn count_beer_tastings(pool: &PgPool, beer_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tastings WHERE beer_id = $1")
        .bind(beer_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Get tastings within a session
pub async fn get_session_tastings(
    pool: &PgPool,
    session_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<TastingWithDetailsRow>, sqlx::Error> {
    sqlx::query_as::<_, TastingWithDetailsRow>(
        r#"
        SELECT t.id, t.user_id, t.beer_id, b.name as beer_name, br.name as brewery_name,
               t.score, t.serving_style, t.notes_encrypted,
               t.location_id, l.name as location_name,
               t.session_id, ts.name as session_name,
               t.tasted_at, t.created_at
        FROM tastings t
        JOIN beers b ON b.id = t.beer_id
        JOIN breweries br ON br.id = b.brewery_id
        LEFT JOIN locations l ON l.id = t.location_id
        LEFT JOIN tasting_sessions ts ON ts.id = t.session_id
        WHERE t.session_id = $1
        ORDER BY t.tasted_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(session_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

// --- Badge support queries (use tastings instead of ratings) ---

pub async fn count_user_tastings(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tastings WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn count_user_unique_beers(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT beer_id) FROM tastings WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

pub async fn count_user_unique_styles(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT b.style)
        FROM tastings t
        JOIN beers b ON b.id = t.beer_id
        WHERE t.user_id = $1 AND b.style IS NOT NULL
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn max_tastings_same_brewery(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT t.beer_id) as cnt
        FROM tastings t
        JOIN beers b ON b.id = t.beer_id
        WHERE t.user_id = $1
        GROUP BY b.brewery_id
        ORDER BY cnt DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0).unwrap_or(0))
}

/// Recent tastings across all users (public feed).
/// Only shows tastings from users who have not opted out of public activity.
pub async fn get_recent_tastings(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<RecentTastingRow>, sqlx::Error> {
    sqlx::query_as::<_, RecentTastingRow>(
        r#"
        SELECT t.id, u.username, t.beer_id, b.name as beer_name, b.style as beer_style,
               br.name as brewery_name, t.score, t.serving_style,
               l.name as location_name, ts.name as session_name,
               t.tasted_at
        FROM tastings t
        JOIN users u ON u.id = t.user_id
        JOIN beers b ON b.id = t.beer_id
        JOIN breweries br ON br.id = b.brewery_id
        LEFT JOIN locations l ON l.id = t.location_id
        LEFT JOIN tasting_sessions ts ON ts.id = t.session_id
        ORDER BY t.tasted_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}
