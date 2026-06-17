use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::errors::{Result, WikiError};
use crate::middleware::WikiUser;
use crate::models::page::{self, PreviewRequest, SavePageRequest};
use crate::services::{link_service, page_service, permission_service};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub ns:    Option<String>,
    pub title: String,
}

fn categories_json(render: &crate::services::wiki_markup::RenderResult) -> Vec<Value> {
    render
        .categories
        .iter()
        .map(|c| json!({ "title": c.title, "slug": c.slug }))
        .collect()
}

pub async fn get_page(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>> {
    let (wiki, role) = permission_service::authorize(&state, wiki_id, user.id).await?;
    let ns = q
        .ns
        .as_deref()
        .and_then(page::canonical_namespace)
        .unwrap_or("Main");
    let title = page::normalize_title(&q.title);
    let slug = page::slugify(&title);

    match page_service::get_rendered(&state, &wiki, ns, &title).await? {
        Some(rp) => Ok(Json(json!({
            "exists":         true,
            "id":             rp.page.id,
            "namespace":      ns,
            "title":          rp.page.title,
            "slug":           rp.page.slug,
            "prefixed_title": page::prefixed_title(ns, &rp.page.title),
            "talk_namespace": page::talk_namespace(ns),
            "redirect":       rp.render.redirect,
            "html":           rp.render.html,
            "toc":            rp.render.toc,
            "categories":     categories_json(&rp.render),
            "source":         rp.source,
            "updated_at":     rp.page.current_rev_at,
            "can_edit":       role.can_edit(),
            "can_admin":      role.can_admin(),
        }))),
        None => Ok(Json(json!({
            "exists":         false,
            "namespace":      ns,
            "title":          title,
            "slug":           slug,
            "prefixed_title": page::prefixed_title(ns, &title),
            "talk_namespace": page::talk_namespace(ns),
            "html":           "",
            "source":         "",
            "categories":     [],
            "can_edit":       role.can_edit(),
            "can_admin":      role.can_admin(),
        }))),
    }
}

pub async fn list_pages(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let pages = page_service::list_pages(&state, wiki_id).await?;
    Ok(Json(json!({ "pages": pages })))
}

pub async fn save_page(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
    Json(req): Json<SavePageRequest>,
) -> Result<Json<Value>> {
    let wiki = permission_service::require_edit(&state, wiki_id, user.id).await?;
    let name = if user.display_name.is_empty() { user.email.clone() } else { user.display_name.clone() };
    let page = page_service::save_page(&state, &wiki, user.id, &name, req).await?;
    Ok(Json(json!({ "page": page })))
}

pub async fn preview_page(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path(wiki_id): Path<Uuid>,
    Json(req): Json<PreviewRequest>,
) -> Result<Json<Value>> {
    let wiki = permission_service::require_edit(&state, wiki_id, user.id).await?;
    let ns = req.namespace.as_deref().and_then(page::canonical_namespace).unwrap_or("Main");
    let title = page::normalize_title(&req.title);
    let render = page_service::preview(&state, &wiki, ns, &title, &req.content).await?;
    Ok(Json(json!({
        "html":       render.html,
        "toc":        render.toc,
        "categories": categories_json(&render),
        "redirect":   render.redirect,
    })))
}

pub async fn delete_page(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path((wiki_id, page_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    let wiki = permission_service::require_edit(&state, wiki_id, user.id).await?;
    page_service::delete_page(&state, &wiki, user.id, page_id).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct MoveRequest {
    pub target: String,
}

pub async fn move_page(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path((wiki_id, page_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<MoveRequest>,
) -> Result<Json<Value>> {
    let wiki = permission_service::require_edit(&state, wiki_id, user.id).await?;
    let page = page_service::move_page(&state, &wiki, user.id, page_id, &req.target).await?;
    Ok(Json(json!({ "page": page })))
}

pub async fn history(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path((wiki_id, page_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    let wiki = permission_service::require_read(&state, wiki_id, user.id).await?;
    let revisions = page_service::history(&state, &wiki, page_id).await?;
    Ok(Json(json!({ "revisions": revisions })))
}

pub async fn revision(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path((wiki_id, page_id, rev_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Value>> {
    let wiki = permission_service::require_read(&state, wiki_id, user.id).await?;
    let rev = page_service::revision_content(&state, &wiki, page_id, rev_id).await?;
    Ok(Json(json!({ "revision": rev })))
}

pub async fn backlinks(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Path((wiki_id, page_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    permission_service::require_read(&state, wiki_id, user.id).await?;
    let links = link_service::backlinks(&state, wiki_id, page_id).await?;
    Ok(Json(json!({ "backlinks": links })))
}

#[derive(Debug, Deserialize)]
pub struct RecentQuery {
    pub limit: Option<i64>,
}

pub async fn recent(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Query(q): Query<RecentQuery>,
) -> Result<Json<Value>> {
    let pages = page_service::recent_pages(&state, user.id, q.limit.unwrap_or(12)).await?;
    Ok(Json(json!({ "pages": pages })))
}

#[derive(Debug, Deserialize)]
pub struct OpenByFileRequest {
    pub file_id: Uuid,
}

pub async fn open_by_file(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Json(req): Json<OpenByFileRequest>,
) -> Result<Json<Value>> {
    let (wiki_id, namespace, title) = page_service::locate_by_file(&state, req.file_id).await?;
    // Ensure the requester may read the target wiki.
    permission_service::require_read(&state, wiki_id, user.id)
        .await
        .map_err(|_| WikiError::NotFound("page".into()))?;
    Ok(Json(json!({ "wiki_id": wiki_id, "namespace": namespace, "title": title })))
}
