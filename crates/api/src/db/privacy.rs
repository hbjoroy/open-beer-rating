use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct PrivacySettingsRow {
    pub user_id: Uuid,
    pub profile_visibility: String,
    pub show_ratings: bool,
    pub show_badges: bool,
    pub show_stats: bool,
    pub updated_at: DateTime<Utc>,
}

pub async fn get_privacy_settings(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<PrivacySettingsRow>, sqlx::Error> {
    sqlx::query_as::<_, PrivacySettingsRow>(
        r#"
        SELECT user_id, profile_visibility::text, show_ratings, show_badges, show_stats, updated_at
        FROM user_privacy_settings
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_privacy_settings(
    pool: &PgPool,
    user_id: Uuid,
    profile_visibility: &str,
    show_ratings: bool,
    show_badges: bool,
    show_stats: bool,
) -> Result<PrivacySettingsRow, sqlx::Error> {
    sqlx::query_as::<_, PrivacySettingsRow>(
        r#"
        UPDATE user_privacy_settings
        SET profile_visibility = $2::profile_visibility,
            show_ratings = $3,
            show_badges = $4,
            show_stats = $5,
            updated_at = now()
        WHERE user_id = $1
        RETURNING user_id, profile_visibility::text, show_ratings, show_badges, show_stats, updated_at
        "#,
    )
    .bind(user_id)
    .bind(profile_visibility)
    .bind(show_ratings)
    .bind(show_badges)
    .bind(show_stats)
    .fetch_one(pool)
    .await
}

pub async fn delete_user_data(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    // Delete in order respecting FK constraints
    // user_badges, ratings, user_privacy_settings all cascade from users
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
