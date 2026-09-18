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

/// This module's id, used as the token audience.
const MODULE_ID: &str = "wiki";

/// Authenticate the caller from the signed `X-Kubuno-Auth` token the core
/// mints with this module's internal secret (see `kubuno-modauth`), instead of
/// trusting the plain `X-Kubuno-User-*` headers — which any process reaching this
/// module's loopback port could forge to impersonate any user.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> std::result::Result<Response, WikiError> {
    let token = req
        .headers()
        .get(kubuno_modauth::TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(WikiError::Unauthorized)?;

    let user = kubuno_modauth::verify(
        state.settings.core.internal_secret.as_bytes(),
        token,
        MODULE_ID,
    )
    .map_err(|_| WikiError::Unauthorized)?;

    req.extensions_mut()
        .insert(WikiUser { id: user.id, role: user.role, email: user.email, display_name: String::new() });
    Ok(next.run(req).await)
}
