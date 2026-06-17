use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{errors::WikiError, state::AppState};

#[derive(Debug, Clone)]
pub struct WikiUser {
    pub id:           Uuid,
    pub role:         String,
    pub email:        String,
    pub display_name: String,
}

impl WikiUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

pub type WikiUserExt = axum::Extension<WikiUser>;

pub async fn require_auth(
    State(_state): State<AppState>,
    mut req: Request,
    next: Next,
) -> std::result::Result<Response, WikiError> {
    let user_id = req
        .headers()
        .get("x-kubuno-user-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(WikiError::Unauthorized)?;

    let role = req
        .headers()
        .get("x-kubuno-user-role")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("user")
        .to_string();

    let email = req
        .headers()
        .get("x-kubuno-user-email")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let display_name = req
        .headers()
        .get("x-kubuno-user-display-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    req.extensions_mut().insert(WikiUser { id: user_id, role, email, display_name });
    Ok(next.run(req).await)
}
