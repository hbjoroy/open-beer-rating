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
    pub show_tasting_sessions: bool,
    pub updated_at: DateTime<Utc>,
}

pub async fn get_privacy_settings(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<PrivacySettingsRow>, sqlx::Error> {
    sqlx::query_as::<_, PrivacySettingsRow>(
        r#"
        SELECT user_id, profile_visibility::text, show_ratings, show_badges, show_stats, show_tasting_sessions, updated_at
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
        RETURNING user_id, profile_visibility::text, show_ratings, show_badges, show_stats, show_tasting_sessions, updated_at
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

/// A single repeatable-read snapshot prevents cross-section inconsistencies during export.
/// It contains only this user's tastings and membership records; session ownership
/// does not grant access to other participants' private notes.
pub struct UserDataExport {
    pub user: super::users::UserRow,
    pub tastings: Vec<ExportTastingRow>,
    pub locations: Vec<ExportLocationRow>,
    pub created_sessions: Vec<ExportSessionRow>,
    pub session_memberships: Vec<ExportMembershipRow>,
    pub badges: Vec<super::badges::UserBadgeRow>,
    pub passkeys: Vec<ExportPasskeyRow>,
    pub settings: PrivacySettingsRow,
    pub identity: Option<crate::identity::LinkedIdentity>,
    pub email_recovery_requests: Vec<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
pub struct ExportTastingRow {
    pub id: Uuid,
    pub beer_id: Uuid,
    pub beer_name: String,
    pub brewery_name: String,
    pub score: i32,
    pub serving_style: Option<open_tappd_domain::models::tasting::ServingStyle>,
    pub notes_encrypted: Option<Vec<u8>>,
    pub location_id: Option<Uuid>,
    pub location_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub session_name: Option<String>,
    pub tasted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub untappd_source_encrypted: Option<Vec<u8>>,
    pub photo_thumb_jpeg: Option<Vec<u8>>,
    pub photo_large_jpeg: Option<Vec<u8>>,
}

#[derive(Debug, FromRow, serde::Serialize)]
pub struct ExportLocationRow {
    pub id: Uuid,
    pub name: String,
    pub location_type: open_tappd_domain::models::location::LocationType,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, serde::Serialize)]
pub struct ExportSessionRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub location_id: Option<Uuid>,
    pub created_by: Uuid,
    pub join_code: String,
    pub visibility: open_tappd_domain::models::tasting_session::SessionVisibility,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub planned_start: Option<DateTime<Utc>>,
    pub planned_end: Option<DateTime<Utc>>,
    pub auto_end_minutes: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, serde::Serialize)]
pub struct ExportMembershipRow {
    pub session_id: Uuid,
    pub session_name: String,
    pub joined_at: DateTime<Utc>,
}

/// Device labels and enrollment dates are account data; credential identifiers,
/// public keys and authenticator counters are deliberately excluded.
#[derive(Debug, FromRow, serde::Serialize)]
pub struct ExportPasskeyRow {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

pub async fn export_user_data(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<UserDataExport>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let user = sqlx::query_as::<_, super::users::UserRow>(
        "SELECT id, username, email_encrypted, recovery_key_hash, display_name, created_at, updated_at, auth_version, email_verified_at, email_verification_source, email_recovery_allowed_until FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(user) = user else {
        tx.commit().await?;
        return Ok(None);
    };
    // No pagination or cap: repeated tastings are part of the user's history.
    // Locations are shared catalog data, including entries contributed by others.
    let tastings = sqlx::query_as::<_, ExportTastingRow>(
        r#"
        SELECT t.id, t.beer_id, b.name AS beer_name, br.name AS brewery_name,
               t.score, t.serving_style, t.notes_encrypted,
               l.id AS location_id, l.name AS location_name,
               t.session_id, ts.name AS session_name,
               t.tasted_at, t.created_at, t.updated_at,
               p.thumb_jpeg AS photo_thumb_jpeg, p.large_jpeg AS photo_large_jpeg,
               uc.source_encrypted AS untappd_source_encrypted
        FROM tastings t
        JOIN beers b ON b.id = t.beer_id
        JOIN breweries br ON br.id = b.brewery_id
        LEFT JOIN locations l ON l.id = t.location_id
        LEFT JOIN tasting_sessions ts ON ts.id = t.session_id
        LEFT JOIN tasting_photos p ON p.tasting_id = t.id
        LEFT JOIN untappd_checkins uc ON uc.tasting_id=t.id AND uc.user_id=$1
        WHERE t.user_id = $1
        ORDER BY t.tasted_at DESC, t.created_at DESC, t.id DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    let locations = sqlx::query_as::<_, ExportLocationRow>(
        "SELECT id, name, location_type, metadata, created_by, is_active, created_at, updated_at FROM locations WHERE created_by = $1 OR id IN (SELECT location_id FROM tastings WHERE user_id=$1 UNION SELECT location_id FROM tasting_sessions WHERE created_by=$1) ORDER BY created_at, id",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    let created_sessions = sqlx::query_as::<_, ExportSessionRow>(
        r#"
        SELECT ts.id, ts.name, ts.description, l.id AS location_id, ts.created_by,
               ts.join_code, ts.visibility, ts.started_at, ts.ended_at,
               ts.planned_start, ts.planned_end, ts.auto_end_minutes, ts.created_at, ts.updated_at
        FROM tasting_sessions ts
        LEFT JOIN locations l ON l.id = ts.location_id
        WHERE ts.created_by = $1 ORDER BY ts.created_at, ts.id
        "#,
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    let session_memberships = sqlx::query_as::<_, ExportMembershipRow>(
        "SELECT sp.session_id, ts.name AS session_name, sp.joined_at FROM session_participants sp JOIN tasting_sessions ts ON ts.id = sp.session_id WHERE sp.user_id = $1 ORDER BY sp.joined_at, sp.session_id",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    let badges = sqlx::query_as::<_, super::badges::UserBadgeRow>(
        "SELECT b.name AS badge_name, b.description AS badge_description, b.icon_url AS badge_icon_url, ub.earned_at FROM user_badges ub JOIN badges b ON b.id = ub.badge_id WHERE ub.user_id = $1 ORDER BY ub.earned_at, ub.badge_id",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    let passkeys = sqlx::query_as::<_, ExportPasskeyRow>(
        "SELECT id, name, created_at FROM user_passkeys WHERE user_id = $1 ORDER BY created_at, id",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    let settings = sqlx::query_as::<_, PrivacySettingsRow>(
        "SELECT user_id, profile_visibility::text, show_ratings, show_badges, show_stats, show_tasting_sessions, updated_at FROM user_privacy_settings WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    let identity = sqlx::query_as::<_,crate::identity::LinkedIdentity>("SELECT issuer,subject,display_name,sproyt_user_id,linked_at FROM external_identities WHERE user_id=$1").bind(user_id).fetch_optional(&mut *tx).await?;
    let email_recovery_requests=sqlx::query_scalar("SELECT requested_at FROM email_recovery_requests WHERE user_id=$1 ORDER BY requested_at").bind(user_id).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(UserDataExport {
        email_recovery_requests,
        identity,
        user,
        tastings,
        locations,
        created_sessions,
        session_memberships,
        badges,
        passkeys,
        settings,
    }))
}

pub async fn delete_user_data(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    // Block new FK references to this account while cleaning up.
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    // Catalog locations survive; their contributor FK becomes NULL.
    sqlx::query("DELETE FROM tasting_sessions WHERE created_by = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    // Own tastings, legacy ratings, badges, memberships, privacy and passkeys cascade.
    // Shared beer, brewery and location catalog entries are retained.
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
