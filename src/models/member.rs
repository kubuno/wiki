use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct WikiMember {
    pub wiki_id:  Uuid,
    pub user_id:  Uuid,
    pub role:     String,
    pub added_at: DateTime<Utc>,
}

/// Member enriched with profile info fetched from the core.
#[derive(Debug, Clone, Serialize)]
pub struct WikiMemberView {
    pub user_id:      Uuid,
    pub role:         String,
    pub display_name: String,
    pub email:        String,
    pub added_at:     DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    /// Email of the user to add (resolved against the core directory).
    pub email: String,
    #[serde(default = "default_role")]
    pub role:  String,
}

fn default_role() -> String { "editor".to_string() }

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub role: String,
}

/// Effective role of a user on a wiki. `Owner` always wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Admin,
    Editor,
    Reader,
    None,
}

impl Role {
    pub fn parse(s: &str) -> Role {
        match s {
            "owner"  => Role::Owner,
            "admin"  => Role::Admin,
            "editor" => Role::Editor,
            "reader" => Role::Reader,
            _        => Role::None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner  => "owner",
            Role::Admin  => "admin",
            Role::Editor => "editor",
            Role::Reader => "reader",
            Role::None   => "none",
        }
    }

    pub fn can_read(self) -> bool {
        !matches!(self, Role::None)
    }

    pub fn can_edit(self) -> bool {
        matches!(self, Role::Owner | Role::Admin | Role::Editor)
    }

    pub fn can_admin(self) -> bool {
        matches!(self, Role::Owner | Role::Admin)
    }
}
