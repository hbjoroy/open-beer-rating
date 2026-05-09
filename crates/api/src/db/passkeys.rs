use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct PasskeyRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Vec<u8>,
    pub public_key_cbor: Vec<u8>,
    pub counter: i32,
    pub transports: Option<String>,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

pub async fn store_passkey(
    pool: &PgPool,
    user_id: Uuid,
    credential_id: &[u8],
    public_key_cbor: &[u8],
    counter: i32,
    transports: Option<&str>,
    name: &str,
) -> Result<PasskeyRow, sqlx::Error> {
    sqlx::query_as::<_, PasskeyRow>(
        r#"
        INSERT INTO user_passkeys (user_id, credential_id, public_key_cbor, counter, transports, name)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, user_id, credential_id, public_key_cbor, counter, transports, name, created_at
        "#,
    )
    .bind(user_id)
    .bind(credential_id)
    .bind(public_key_cbor)
    .bind(counter)
    .bind(transports)
    .bind(name)
    .fetch_one(pool)
    .await
}

pub async fn list_passkeys(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<PasskeyRow>, sqlx::Error> {
    sqlx::query_as::<_, PasskeyRow>(
        r#"
        SELECT id, user_id, credential_id, public_key_cbor, counter, transports, name, created_at
        FROM user_passkeys
        WHERE user_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn find_passkey_by_credential_id(
    pool: &PgPool,
    credential_id: &[u8],
) -> Result<Option<PasskeyRow>, sqlx::Error> {
    sqlx::query_as::<_, PasskeyRow>(
        r#"
        SELECT id, user_id, credential_id, public_key_cbor, counter, transports, name, created_at
        FROM user_passkeys
        WHERE credential_id = $1
        "#,
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_passkey_counter(
    pool: &PgPool,
    id: Uuid,
    counter: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE user_passkeys SET counter = $1 WHERE id = $2")
        .bind(counter)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_passkey(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM user_passkeys WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn count_passkeys(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM user_passkeys WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}
