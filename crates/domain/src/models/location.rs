use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "location_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LocationType {
    Bar,
    Restaurant,
    BreweryTaproom,
    Festival,
    Home,
    Online,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub id: Uuid,
    pub name: String,
    pub location_type: LocationType,
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
