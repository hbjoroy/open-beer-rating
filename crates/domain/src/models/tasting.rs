use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tasting {
    pub id: Uuid,
    pub user_id: Uuid,
    pub beer_id: Uuid,
    pub score: i32,
    pub location_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub tasted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // notes_encrypted not exposed in domain model — handled at API layer
}
