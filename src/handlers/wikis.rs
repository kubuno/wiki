use axum::{
    extract::{Path, State},
    Extension, Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::errors::Result;
use crate::middleware::WikiUser;
use crate::models::member::{AddMemberRequest, UpdateMemberRequest};
use crate::models::wiki::{CreateWikiRequest, UpdateWikiRequest};
use crate::services::wiki_service;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
) -> Result<Json<Value>> {
    let wikis = wiki_service::list_wikis(&state, user.id).await?;
    Ok(Json(json!({ "wikis": wikis })))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Json(req): Json<CreateWikiRequest>,
) -> Result<Json<Value>> {
    let wiki = wiki_service::create_wiki(&state, user.id, req).await?;
    Ok(Json(json!({ "wiki": wiki })))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let wiki = wiki_service::get_wiki_view(&state, id, user.id).await?;
    Ok(Json(json!({ "wiki": wiki })))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWikiRequest>,
) -> Result<Json<Value>> {
    let wiki = wiki_service::update_wiki(&state, id, user.id, req).await?;
    Ok(Json(json!({ "wiki": wiki })))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    wiki_service::delete_wiki(&state, id, user.id).await?;
    Ok(Json(json!({ "ok": true })))
}

// ── Members ─────────────────────────────────────────────────────────────────

pub async fn list_members(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let members = wiki_service::list_members(&state, id, user.id).await?;
    Ok(Json(json!({ "members": members })))
}

pub async fn add_member(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> Result<Json<Value>> {
    wiki_service::add_member(&state, id, user.id, &req.email, &req.role).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn update_member(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path((id, member_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateMemberRequest>,
) -> Result<Json<Value>> {
    wiki_service::update_member(&state, id, user.id, member_id, &req.role).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove_member(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path((id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    wiki_service::remove_member(&state, id, user.id, member_id).await?;
    Ok(Json(json!({ "ok": true })))
}
