use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct BreweryRow {
    pub id: Uuid,
    pub name: String,
    pub country: Option<String>,
    pub city: Option<String>,
    pub website: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn create_brewery(
    pool: &PgPool,
    name: &str,
    country: Option<&str>,
    city: Option<&str>,
    website: Option<&str>,
) -> Result<BreweryRow, sqlx::Error> {
    sqlx::query_as::<_, BreweryRow>(
        r#"
        INSERT INTO breweries (name, country, city, website)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, country, city, website, created_at
        "#,
    )
    .bind(name)
    .bind(country)
    .bind(city)
    .bind(website)
    .fetch_one(pool)
    .await
}

pub async fn list_breweries(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<BreweryRow>, sqlx::Error> {
    sqlx::query_as::<_, BreweryRow>(
        "SELECT id, name, country, city, website, created_at FROM breweries ORDER BY name LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn get_brewery(pool: &PgPool, id: Uuid) -> Result<Option<BreweryRow>, sqlx::Error> {
    sqlx::query_as::<_, BreweryRow>(
        "SELECT id, name, country, city, website, created_at FROM breweries WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}
