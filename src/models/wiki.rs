use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A wiki space. Personal wikis store their `.kbwik` files under the owner's
/// drive; shared wikis store them under the reserved system user.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Wiki {
    pub id:               Uuid,
    pub owner_id:         Uuid,
    #[serde(skip_serializing)]
    pub storage_owner_id: Uuid,
    pub slug:             String,
    pub name:             String,
    pub description:      String,
    pub is_shared:        bool,
    pub created_at:       DateTime<Utc>,
    pub updated_at:       DateTime<Utc>,
}

/// Wiki plus the requesting user's effective role and page count.
#[derive(Debug, Clone, Serialize)]
pub struct WikiView {
    #[serde(flatten)]
    pub wiki:       Wiki,
    pub my_role:    String,
    pub page_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateWikiRequest {
    pub name:        String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_shared:   bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWikiRequest {
    pub name:        Option<String>,
    pub description: Option<String>,
}
