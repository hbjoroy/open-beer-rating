use chrono::{DateTime, Utc};
use open_tappd_domain::models::location::LocationType;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct LocationRow {
    pub id: Uuid,
    pub name: String,
    pub location_type: LocationType,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_location(
    pool: &PgPool,
    name: &str,
    location_type: LocationType,
    metadata: &serde_json::Value,
    created_by: Uuid,
) -> Result<LocationRow, sqlx::Error> {
    let mut tx = pool.begin().await?;
    // Match the user-before-catalog lock order used by imports and deletion.
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR KEY SHARE")
        .bind(created_by)
        .execute(&mut *tx)
        .await?;
    sqlx::query("LOCK TABLE locations IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let existing = sqlx::query_as::<_, LocationRow>(
        "SELECT id,name,location_type,metadata,created_by,is_active,created_at,updated_at FROM locations WHERE catalog_key(name)=catalog_key($1) AND catalog_key(coalesce(metadata->>'city',''))=catalog_key(coalesce($2,'')) AND catalog_key(coalesce(metadata->>'country',''))=catalog_key(coalesce($3,'')) ORDER BY is_active DESC,created_at,id LIMIT 1"
    ).bind(name).bind(metadata.get("city").and_then(|v|v.as_str()))
        .bind(metadata.get("country").and_then(|v|v.as_str())).fetch_optional(&mut *tx).await?;
    if let Some(row) = existing {
        tx.commit().await?;
        return Ok(row);
    }
    let row = sqlx::query_as::<_, LocationRow>(
        "INSERT INTO locations (name,location_type,metadata,created_by) VALUES($1,$2,$3,$4) RETURNING id,name,location_type,metadata,created_by,is_active,created_at,updated_at"
    ).bind(name.trim()).bind(location_type).bind(metadata).bind(created_by).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn get_location(pool: &PgPool, id: Uuid) -> Result<Option<LocationRow>, sqlx::Error> {
    sqlx::query_as::<_, LocationRow>(
        r#"
        SELECT id, name, location_type, metadata, created_by, is_active, created_at, updated_at
        FROM locations WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_locations(
    pool: &PgPool,
    location_type_filter: Option<LocationType>,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<LocationRow>, sqlx::Error> {
    sqlx::query_as::<_, LocationRow>(
        r#"
        SELECT id, name, location_type, metadata, created_by, is_active, created_at, updated_at
        FROM locations
        WHERE is_active = true
          AND ($1::location_type IS NULL OR location_type = $1)
          AND ($2::text IS NULL OR name ILIKE '%' || $2 || '%' OR metadata->>'city' ILIKE '%' || $2 || '%')
        ORDER BY name, id
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(location_type_filter)
    .bind(search)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn update_location(
    pool: &PgPool,
    id: Uuid,
    creator_id: Uuid,
    name: &str,
    location_type: LocationType,
    metadata: &serde_json::Value,
) -> Result<Option<LocationRow>, sqlx::Error> {
    sqlx::query_as::<_, LocationRow>(
        r#"
        UPDATE locations
        SET name = $3, location_type = $4, metadata = $5, updated_at = now()
        WHERE id = $1 AND created_by = $2
        RETURNING id, name, location_type, metadata, created_by, is_active, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(creator_id)
    .bind(name)
    .bind(location_type)
    .bind(metadata)
    .fetch_optional(pool)
    .await
}

/// Soft-delete: set is_active = false (creator only)
pub async fn soft_delete_location(
    pool: &PgPool,
    id: Uuid,
    creator_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE locations SET is_active = false, updated_at = now() WHERE id = $1 AND created_by = $2",
    )
    .bind(id)
    .bind(creator_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
