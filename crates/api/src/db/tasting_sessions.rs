use chrono::{DateTime, Utc};
use open_tappd_domain::models::tasting_session::SessionVisibility;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct TastingSessionRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub location_id: Option<Uuid>,
    pub created_by: Uuid,
    pub join_code: String,
    pub visibility: SessionVisibility,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub planned_start: Option<DateTime<Utc>>,
    pub planned_end: Option<DateTime<Utc>>,
    pub auto_end_minutes: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct SessionParticipantRow {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub joined_at: DateTime<Utc>,
}

/// Generate an unpredictable invitation code using the existing UUID entropy source.
fn generate_join_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    uuid::Uuid::new_v4().as_bytes()[..6]
        .iter()
        .map(|byte| CHARSET[usize::from(*byte) % CHARSET.len()] as char)
        .collect()
}
pub async fn create_session(
    pool: &PgPool,
    name: &str,
    description: Option<&str>,
    location_id: Option<Uuid>,
    created_by: Uuid,
    visibility: SessionVisibility,
    auto_end_minutes: i32,
    planned_start: Option<DateTime<Utc>>,
    planned_end: Option<DateTime<Utc>>,
) -> Result<TastingSessionRow, sqlx::Error> {
    // Try generating join codes until we get a unique one (collision unlikely)
    let mut attempts = 0;
    loop {
        let join_code = generate_join_code();
        let result = sqlx::query_as::<_, TastingSessionRow>(
            r#"
            INSERT INTO tasting_sessions
                (name, description, location_id, created_by, join_code, visibility,
                 auto_end_minutes, planned_start, planned_end)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, name, description, location_id, created_by, join_code, visibility,
                      started_at, ended_at, planned_start, planned_end, auto_end_minutes,
                      created_at, updated_at
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(location_id)
        .bind(created_by)
        .bind(&join_code)
        .bind(visibility)
        .bind(auto_end_minutes)
        .bind(planned_start)
        .bind(planned_end)
        .fetch_one(pool)
        .await;

        match result {
            Ok(row) => return Ok(row),
            Err(sqlx::Error::Database(ref db_err))
                if db_err.constraint() == Some("tasting_sessions_join_code_key") =>
            {
                attempts += 1;
                if attempts >= 5 {
                    return Err(result.unwrap_err());
                }
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

pub async fn get_session(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<TastingSessionRow>, sqlx::Error> {
    sqlx::query_as::<_, TastingSessionRow>(
        r#"
        SELECT id, name, description,
               CASE WHEN EXISTS (SELECT 1 FROM locations l WHERE l.id = tasting_sessions.location_id
                                 AND l.created_by = tasting_sessions.created_by)
                    THEN location_id END AS location_id, created_by, join_code, visibility,
               started_at, ended_at, planned_start, planned_end, auto_end_minutes,
               created_at, updated_at
        FROM tasting_sessions WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_session_by_code(
    pool: &PgPool,
    join_code: &str,
) -> Result<Option<TastingSessionRow>, sqlx::Error> {
    sqlx::query_as::<_, TastingSessionRow>(
        r#"
        SELECT id, name, description,
               CASE WHEN EXISTS (SELECT 1 FROM locations l WHERE l.id = tasting_sessions.location_id
                                 AND l.created_by = tasting_sessions.created_by)
                    THEN location_id END AS location_id, created_by, join_code, visibility,
               started_at, ended_at, planned_start, planned_end, auto_end_minutes,
               created_at, updated_at
        FROM tasting_sessions WHERE join_code = $1
        "#,
    )
    .bind(join_code)
    .fetch_optional(pool)
    .await
}

/// Session context is only visible to its owner and participants.
pub async fn list_sessions(
    pool: &PgPool,
    user_id: Uuid,
    active_only: bool,
    limit: i64,
    offset: i64,
) -> Result<Vec<TastingSessionRow>, sqlx::Error> {
    sqlx::query_as::<_, TastingSessionRow>(
        r#"
        SELECT DISTINCT ts.id, ts.name, ts.description, l.id AS location_id, ts.created_by,
               ts.join_code, ts.visibility, ts.started_at, ts.ended_at,
               ts.planned_start, ts.planned_end, ts.auto_end_minutes,
               ts.created_at, ts.updated_at
        FROM tasting_sessions ts
        LEFT JOIN locations l ON l.id = ts.location_id AND l.created_by = ts.created_by
        LEFT JOIN session_participants sp ON sp.session_id = ts.id AND sp.user_id = $1
        WHERE (ts.created_by = $1 OR sp.user_id IS NOT NULL)
          AND ($2::bool = false OR ts.ended_at IS NULL)
        ORDER BY ts.started_at DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(user_id)
    .bind(active_only)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn end_session(pool: &PgPool, id: Uuid, creator_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE tasting_sessions
        SET ended_at = now(), updated_at = now()
        WHERE id = $1 AND created_by = $2 AND ended_at IS NULL
        "#,
    )
    .bind(id)
    .bind(creator_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

// --- Participants ---

pub async fn join_session(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO session_participants (session_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(session_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn leave_session(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM session_participants WHERE session_id = $1 AND user_id = $2")
            .bind(session_id)
            .bind(user_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn is_participant(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM session_participants WHERE session_id = $1 AND user_id = $2")
            .bind(session_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}

pub async fn get_participants(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<Vec<SessionParticipantRow>, sqlx::Error> {
    sqlx::query_as::<_, SessionParticipantRow>(
        r#"
        SELECT sp.session_id, sp.user_id, u.username, sp.joined_at
        FROM session_participants sp
        JOIN users u ON u.id = sp.user_id
        WHERE sp.session_id = $1
        ORDER BY sp.joined_at
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
}
