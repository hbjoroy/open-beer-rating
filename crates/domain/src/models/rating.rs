use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    pub id: Uuid,
    pub user_id: Uuid,
    pub beer_id: Uuid,
    pub score: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingAggregate {
    pub beer_id: Uuid,
    pub average_score: f64,
    pub rating_count: i64,
}
