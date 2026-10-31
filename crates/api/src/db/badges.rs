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

pub async fn has_badge(pool: &PgPool, user_id: Uuid, badge_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_badges WHERE user_id = $1 AND badge_id = $2)",
    )
    .bind(user_id)
    .bind(badge_id)
    .fetch_one(pool)
    .await
}

pub async fn award_badge(pool: &PgPool, user_id: Uuid, badge_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_badges (user_id, badge_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(badge_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Only the drinker's own venue references qualify. A session's venue or the
/// beer/brewery name does not establish where the tasting happened.
pub async fn has_saint_pub_tasting(pool: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    use open_tappd_domain::models::{badge::is_saint_pub, location::LocationType};

    let venues: Vec<(String, LocationType)> = sqlx::query_as(
        "SELECT DISTINCT l.name, l.location_type FROM tastings t \
         JOIN locations l ON l.id = t.location_id \
         WHERE t.user_id = $1 AND l.location_type IN ('bar', 'other')",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(venues
        .into_iter()
        .any(|(name, kind)| is_saint_pub(&name, kind)))
}
