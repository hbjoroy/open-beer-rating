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
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO ratings (user_id, beer_id, score, notes_encrypted)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (user_id, beer_id)
        DO UPDATE SET score = $3, notes_encrypted = $4, created_at = now()
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(beer_id)
    .bind(score)
    .bind(notes_encrypted)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
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
        FROM ratings
        WHERE beer_id = $1
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
        SELECT r.id, r.beer_id, b.name as beer_name, br.name as brewery_name,
               r.score, r.notes_encrypted, r.created_at
        FROM ratings r
        JOIN beers b ON b.id = r.beer_id
        JOIN breweries br ON br.id = b.brewery_id
        WHERE r.user_id = $1
        ORDER BY r.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_user_ratings(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ratings WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn count_user_unique_styles(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT b.style)
        FROM ratings r
        JOIN beers b ON b.id = r.beer_id
        WHERE r.user_id = $1 AND b.style IS NOT NULL
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn max_ratings_same_brewery(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT COUNT(*) as cnt
        FROM ratings r
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
