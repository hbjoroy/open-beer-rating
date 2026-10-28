use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct RatingAggregateRow {
    pub average_score: Option<f64>,
    pub rating_count: Option<i64>,
}

#[derive(Debug, FromRow)]
pub struct UserRatingRow {
    pub id: Uuid,
    pub beer_id: Uuid,
    pub beer_name: String,
    pub brewery_name: String,
    pub score: i32,
    pub notes_encrypted: Option<Vec<u8>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn upsert_rating(
    pool: &PgPool,
    user_id: Uuid,
    beer_id: Uuid,
    score: i32,
    notes_encrypted: Option<&[u8]>,
) -> Result<Uuid, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    // Serialize legacy upserts even when no tasting exists yet. Hash collisions
    // only serialize unrelated pairs; they cannot mix their data.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("legacy-rating:{user_id}:{beer_id}"))
        .execute(&mut *transaction)
        .await?;

    let existing: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM tastings
        WHERE user_id = $1 AND beer_id = $2
        ORDER BY tasted_at DESC, created_at DESC, id DESC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .bind(beer_id)
    .fetch_optional(&mut *transaction)
    .await?;

    let id = if let Some((id,)) = existing {
        sqlx::query(
            "UPDATE tastings SET score = $2, notes_encrypted = $3, updated_at = now() WHERE id = $1",
        )
        .bind(id)
        .bind(score)
        .bind(notes_encrypted)
        .execute(&mut *transaction)
        .await?;
        id
    } else {
        let row: (Uuid,) = sqlx::query_as(
            r#"
        INSERT INTO tastings (user_id, beer_id, score, notes_encrypted)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        )
        .bind(user_id)
        .bind(beer_id)
        .bind(score)
        .bind(notes_encrypted)
        .fetch_one(&mut *transaction)
        .await?;
        row.0
    };
    transaction.commit().await?;
    Ok(id)
}

pub async fn get_beer_aggregate(
    pool: &PgPool,
    beer_id: Uuid,
) -> Result<RatingAggregateRow, sqlx::Error> {
    sqlx::query_as::<_, RatingAggregateRow>(
        r#"
        SELECT
            AVG(score::float) as average_score,
            COUNT(*) as rating_count
        FROM (
            SELECT DISTINCT ON (user_id) score
            FROM tastings
            WHERE beer_id = $1
            ORDER BY user_id, tasted_at DESC, created_at DESC, id DESC
        ) latest
        "#,
    )
    .bind(beer_id)
    .fetch_one(pool)
    .await
}

pub async fn get_user_ratings(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<UserRatingRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRatingRow>(
        r#"
        WITH latest AS (
            SELECT DISTINCT ON (beer_id) *
            FROM tastings
            WHERE user_id = $1
            ORDER BY beer_id, tasted_at DESC, created_at DESC, id DESC
        )
        SELECT r.id, r.beer_id, b.name as beer_name, br.name as brewery_name,
               r.score, r.notes_encrypted, r.tasted_at AS created_at
        FROM latest r
        JOIN beers b ON b.id = r.beer_id
        JOIN breweries br ON br.id = b.brewery_id
        WHERE r.user_id = $1
        ORDER BY r.tasted_at DESC, r.created_at DESC, r.id DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Legacy badge counter: count all tasting events, including repeated beers.
pub async fn count_user_ratings(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tastings WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn count_user_unique_styles(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT b.style)
        FROM tastings r
        JOIN beers b ON b.id = r.beer_id
        WHERE r.user_id = $1 AND b.style IS NOT NULL
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Brewery badges count distinct beers, so repeat tastings do not inflate them.
pub async fn max_ratings_same_brewery(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT r.beer_id) as cnt
        FROM tastings r
        JOIN beers b ON b.id = r.beer_id
        WHERE r.user_id = $1
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
