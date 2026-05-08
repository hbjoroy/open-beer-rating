use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beer {
    pub id: Uuid,
    pub brewery_id: Uuid,
    pub name: String,
    pub style: Option<String>,
    pub abv: Option<f64>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}
