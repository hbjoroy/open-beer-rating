use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct BeerRow {
    pub id: Uuid,
    pub brewery_id: Uuid,
    pub name: String,
    pub style: Option<String>,
    pub abv: Option<f64>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn create_beer(
    pool: &PgPool,
    brewery_id: Uuid,
    name: &str,
    style: Option<&str>,
    abv: Option<f64>,
    description: Option<&str>,
) -> Result<BeerRow, sqlx::Error> {
    sqlx::query_as::<_, BeerRow>(
        r#"
        INSERT INTO beers (brewery_id, name, style, abv, description)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, brewery_id, name, style, abv, description, created_at
        "#,
    )
    .bind(brewery_id)
    .bind(name)
    .bind(style)
    .bind(abv)
    .bind(description)
    .fetch_one(pool)
    .await
}

pub async fn list_beers(
    pool: &PgPool,
    brewery_id: Option<Uuid>,
    style: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<BeerRow>, sqlx::Error> {
    sqlx::query_as::<_, BeerRow>(
        r#"
        SELECT id, brewery_id, name, style, abv, description, created_at
        FROM beers
        WHERE ($1::uuid IS NULL OR brewery_id = $1)
          AND ($2::text IS NULL OR style ILIKE $2)
        ORDER BY name
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(brewery_id)
    .bind(style)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn get_beer(pool: &PgPool, id: Uuid) -> Result<Option<BeerRow>, sqlx::Error> {
    sqlx::query_as::<_, BeerRow>(
        "SELECT id, brewery_id, name, style, abv, description, created_at FROM beers WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}
