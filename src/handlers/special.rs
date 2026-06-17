use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::errors::Result;
use crate::middleware::WikiUser;
use crate::services::{permission_service, special_service};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct NsQuery {
    pub ns: Option<String>,
}

pub async fn all_pages(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
    Query(q): Query<NsQuery>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let ns = q.ns.as_deref().and_then(crate::models::page::canonical_namespace);
    let pages = special_service::all_pages(&state, wiki_id, ns).await?;
    Ok(Json(json!({ "pages": pages })))
}

#[derive(Debug, Deserialize)]
pub struct LimitQuery {
    pub limit: Option<i64>,
}

pub async fn recent_changes(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let changes = special_service::recent_changes(&state, wiki_id, q.limit.unwrap_or(100)).await?;
    Ok(Json(json!({ "changes": changes })))
}

pub async fn wanted_pages(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let pages = special_service::wanted_pages(&state, wiki_id).await?;
    Ok(Json(json!({ "pages": pages })))
}

pub async fn orphaned_pages(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let pages = special_service::orphaned_pages(&state, wiki_id).await?;
    Ok(Json(json!({ "pages": pages })))
}

pub async fn categories(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let cats = special_service::categories(&state, wiki_id).await?;
    Ok(Json(json!({ "categories": cats })))
}

pub async fn category_members(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path((wiki_id, slug)): Path<(Uuid, String)>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let pages = special_service::category_members(&state, wiki_id, &slug).await?;
    Ok(Json(json!({ "pages": pages })))
}

/// Static list of canonical namespaces (no wiki context needed).
pub async fn namespaces() -> Json<Value> {
    Json(json!({ "namespaces": crate::models::page::NAMESPACES }))
}
