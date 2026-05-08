use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::privacy::ProfileVisibility;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPrivacySettings {
    pub user_id: Uuid,
    pub profile_visibility: ProfileVisibility,
    pub show_ratings: bool,
    pub show_badges: bool,
    pub show_stats: bool,
    pub updated_at: DateTime<Utc>,
}

impl Default for UserPrivacySettings {
    fn default() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            profile_visibility: ProfileVisibility::Private,
            show_ratings: false,
            show_badges: false,
            show_stats: false,
            updated_at: Utc::now(),
        }
    }
}
