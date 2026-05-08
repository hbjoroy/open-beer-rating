use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brewery {
    pub id: Uuid,
    pub name: String,
    pub country: Option<String>,
    pub city: Option<String>,
    pub website: Option<String>,
    pub created_at: DateTime<Utc>,
}
