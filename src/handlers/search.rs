use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::errors::Result;
use crate::middleware::WikiUser;
use crate::services::{permission_service, search_service};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q:     String,
    pub limit: Option<i64>,
}

pub async fn search(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let hits = search_service::search(&state, wiki_id, &q.q, q.limit.unwrap_or(30)).await?;
    Ok(Json(json!({ "results": hits })))
}
