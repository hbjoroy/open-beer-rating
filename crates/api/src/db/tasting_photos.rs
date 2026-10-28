use sqlx::PgPool;
use uuid::Uuid;

pub async fn is_owner(pool: &PgPool, tasting_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tastings WHERE id = $1 AND user_id = $2)")
        .bind(tasting_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
}

pub async fn save(
    pool: &PgPool,
    tasting_id: Uuid,
    user_id: Uuid,
    thumb: &[u8],
    large: &[u8],
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO tasting_photos (tasting_id, thumb_jpeg, large_jpeg) \
         SELECT id, $3, $4 FROM tastings WHERE id = $1 AND user_id = $2 \
         ON CONFLICT (tasting_id) DO UPDATE SET thumb_jpeg = EXCLUDED.thumb_jpeg, \
         large_jpeg = EXCLUDED.large_jpeg, updated_at = now()",
    )
    .bind(tasting_id)
    .bind(user_id)
    .bind(thumb)
    .bind(large)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn get(
    pool: &PgPool,
    tasting_id: Uuid,
    user_id: Uuid,
    thumbnail: bool,
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT CASE WHEN $3 THEN p.thumb_jpeg ELSE p.large_jpeg END \
         FROM tasting_photos p JOIN tastings t ON t.id = p.tasting_id \
         WHERE t.id = $1 AND t.user_id = $2",
    )
    .bind(tasting_id)
    .bind(user_id)
    .bind(thumbnail)
    .fetch_optional(pool)
    .await
}

pub async fn delete(pool: &PgPool, tasting_id: Uuid, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM tasting_photos p USING tastings t \
         WHERE p.tasting_id = t.id AND t.id = $1 AND t.user_id = $2",
    )
    .bind(tasting_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
