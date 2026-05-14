use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub email_encrypted: Option<Vec<u8>>,
    pub recovery_key_hash: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_user(
    pool: &PgPool,
    username: &str,
    email_encrypted: Option<&[u8]>,
    recovery_key_hash: &str,
) -> Result<UserRow, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        r#"
        INSERT INTO users (username, email_encrypted, recovery_key_hash)
        VALUES ($1, $2, $3)
        RETURNING id, username, email_encrypted, recovery_key_hash, display_name, created_at, updated_at
        "#,
    )
    .bind(username)
    .bind(email_encrypted)
    .bind(recovery_key_hash)
    .fetch_one(pool)
    .await?;

    // Create default privacy settings (all private)
    sqlx::query("INSERT INTO user_privacy_settings (user_id) VALUES ($1)")
        .bind(row.id)
        .execute(pool)
        .await?;

    Ok(row)
}

pub async fn find_user_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        r#"
        SELECT id, username, email_encrypted, recovery_key_hash, display_name, created_at, updated_at
        FROM users
        WHERE LOWER(username) = LOWER($1)
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_by_id(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        r#"
        SELECT id, username, email_encrypted, recovery_key_hash, display_name, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}
