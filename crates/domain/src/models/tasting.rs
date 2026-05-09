use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx-support", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx-support", sqlx(type_name = "serving_style", rename_all = "snake_case"))]
#[serde(rename_all = "snake_case")]
pub enum ServingStyle {
    Draft,
    Bottle,
    Can,
    Cask,
    Crowler,
    Growler,
    Nitro,
    Taster,
    Other,
}

impl ServingStyle {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Bottle => "Bottle",
            Self::Can => "Can",
            Self::Cask => "Cask",
            Self::Crowler => "Crowler",
            Self::Growler => "Growler",
            Self::Nitro => "Nitro",
            Self::Taster => "Taster",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tasting {
    pub id: Uuid,
    pub user_id: Uuid,
    pub beer_id: Uuid,
    pub score: i32,
    pub serving_style: Option<ServingStyle>,
    pub location_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub tasted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // notes_encrypted not exposed in domain model — handled at API layer
}
