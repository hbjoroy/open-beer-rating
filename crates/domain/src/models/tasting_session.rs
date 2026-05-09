use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "session_visibility", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SessionVisibility {
    Private,
    Participants,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TastingSession {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionParticipant {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
}
