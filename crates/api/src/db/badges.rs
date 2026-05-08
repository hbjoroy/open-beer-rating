use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct BadgeRow {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub criteria_type: String,
    pub criteria_value: i32,
}

#[derive(Debug, FromRow)]
pub struct UserBadgeRow {
    pub badge_name: String,
    pub badge_description: String,
    pub badge_icon_url: Option<String>,
    pub earned_at: DateTime<Utc>,
}

pub async fn get_all_badges(pool: &PgPool) -> Result<Vec<BadgeRow>, sqlx::Error> {
    sqlx::query_as::<_, BadgeRow>(
        "SELECT id, name, description, icon_url, criteria_type, criteria_value FROM badges",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_user_badges(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<UserBadgeRow>, sqlx::Error> {
    sqlx::query_as::<_, UserBadgeRow>(
        r#"
        SELECT b.name as badge_name, b.description as badge_description,
               b.icon_url as badge_icon_url, ub.earned_at
        FROM user_badges ub
        JOIN badges b ON b.id = ub.badge_id
        WHERE ub.user_id = $1
        ORDER BY ub.earned_at
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn has_badge(
    pool: &PgPool,
    user_id: Uuid,
    badge_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM user_badges WHERE user_id = $1 AND badge_id = $2",
    )
    .bind(user_id)
    .bind(badge_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

pub async fn award_badge(
    pool: &PgPool,
    user_id: Uuid,
    badge_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO user_badges (user_id, badge_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(user_id)
        .bind(badge_id)
        .execute(pool)
        .await?;
    Ok(())
}
