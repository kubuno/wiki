//! Sync deltas for the local-first pull (wikis / pages). Owner-scoped changes
//! past `cursor` (monotonic change_seq). Wiki changes inline their `members`;
//! page changes inline the `.kbwik` envelope (source + cached HTML + revisions)
//! and their category rows, so pages read offline. Pages are soft-deleted
//! (is_deleted flag) → no page tombstones; only wikis tombstone on hard delete.
//!
//! The wiki change feed comes from `kubuno_db::journal::changes_since` (the
//! portable `live UNION ALL tombstones`); pages are owner-scoped through a join
//! to `wikis` and carry no tombstone, so they are read directly. The row bodies
//! are reselected as typed structs and serialised in Rust — no PostgreSQL-only
//! `to_jsonb`.
//!
//! Scope: personal wikis (owner = requester) and their pages. Shared-wiki sync
//! is a follow-up.

use axum::{
    extract::{Query, State},
    Extension, Json,
};
use kubuno_db::params;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::errors::Result;
use crate::middleware::WikiUser;
use crate::models::wiki::Wiki;
use crate::services::content_files;
use crate::state::AppState;
use crate::sync;

#[derive(serde::Deserialize)]
pub struct DeltaQuery {
    #[serde(default)]
    cursor: i64,
    limit: Option<i64>,
}

/// One `wiki_members` row, reselected for a wiki delta entry.
#[derive(sqlx::FromRow)]
struct MemberRow {
    user_id:  Uuid,
    role:     String,
    added_at: chrono::DateTime<chrono::Utc>,
}

/// One `page_categories` row, reselected for a page delta entry.
#[derive(sqlx::FromRow)]
struct CategoryRow {
    title: String,
    slug:  String,
}

/// GET /wikis/delta — the requester's wikis + inline members, with tombstones.
pub async fn wikis_delta(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Query(q): Query<DeltaQuery>,
) -> Result<Json<Value>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let feed = kubuno_db::journal::changes_since(
        &state.db, sync::WIKIS_TABLE, sync::WIKI_TOMBSTONES, user.id, q.cursor, limit,
    )
    .await?;
    let has_more = feed.len() as i64 == limit;
    let new_cursor = feed.last().map(|c| c.change_seq).unwrap_or(q.cursor);

    let mut changes = Vec::with_capacity(feed.len());
    for c in &feed {
        if c.deleted {
            changes.push(json!({ "uuid": c.id, "kind": "deleted", "change_seq": c.change_seq }));
            continue;
        }
        let Some(wiki) = state
            .db
            .fetch_optional_as::<Wiki>("SELECT * FROM wiki.wikis WHERE id = $1", params![c.id])
            .await?
        else {
            continue;
        };
        let members = state
            .db
            .fetch_all_as::<MemberRow>(
                "SELECT user_id, role, added_at FROM wiki.wiki_members WHERE wiki_id = $1",
                params![c.id],
            )
            .await?;
        let members: Vec<Value> = members
            .into_iter()
            .map(|m| json!({ "user_id": m.user_id, "role": m.role, "added_at": m.added_at }))
            .collect();
        // The `storage_owner_id` is skipped by `Wiki`'s Serialize, but the client
        // needs it; build the object explicitly so the delta keeps its shape.
        changes.push(json!({
            "uuid": c.id,
            "kind": "modified",
            "change_seq": c.change_seq,
            "wiki": {
                "id": wiki.id,
                "owner_id": wiki.owner_id,
                "storage_owner_id": wiki.storage_owner_id,
                "slug": wiki.slug,
                "name": wiki.name,
                "description": wiki.description,
                "is_shared": wiki.is_shared,
                "created_at": wiki.created_at,
                "updated_at": wiki.updated_at,
            },
            "members": members,
        }));
    }
    Ok(Json(json!({ "changes": changes, "cursor": new_cursor, "has_more": has_more })))
}

/// Page id + its versioned seq + its wiki's storage owner (to read the file).
#[derive(sqlx::FromRow)]
struct PageDeltaRow {
    id:               Uuid,
    change_seq:       i64,
    storage_owner_id: Uuid,
}

/// GET /pages/delta — pages of the requester's wikis, each with its inline
/// `.kbwik` envelope and category rows. Soft-deleted pages ride as `is_deleted`.
pub async fn pages_delta(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Query(q): Query<DeltaQuery>,
) -> Result<Json<Value>> {
    let limit = q.limit.unwrap_or(100).clamp(1, 300);
    // page + its wiki's storage owner (to read the .kbwik file), owner-scoped.
    let rows: Vec<PageDeltaRow> = state
        .db
        .fetch_all_as::<PageDeltaRow>(
            "SELECT p.id, p.change_seq, w.storage_owner_id \
             FROM wiki.pages p JOIN wiki.wikis w ON w.id = p.wiki_id \
             WHERE w.owner_id = $1 AND p.change_seq > $2 \
             ORDER BY p.change_seq LIMIT $3",
            params![user.id, q.cursor, limit],
        )
        .await?;
    let has_more = rows.len() as i64 == limit;
    let new_cursor = rows.last().map(|r| r.change_seq).unwrap_or(q.cursor);

    let mut changes = Vec::with_capacity(rows.len());
    for r in &rows {
        let Some(page) = state
            .db
            .fetch_optional_as::<crate::models::page::Page>(
                "SELECT * FROM wiki.pages WHERE id = $1",
                params![r.id],
            )
            .await?
        else {
            continue;
        };
        // Read the .kbwik envelope (source + cached HTML + revisions). Best-effort:
        // a missing file yields an empty envelope rather than failing the page.
        let content = match content_files::read_page_file(&state, r.storage_owner_id, page.file_id).await {
            Ok(env) => json!({
                "version": env.version, "namespace": env.namespace, "title": env.title,
                "content": env.content, "content_html": env.content_html,
                "redirect": env.redirect, "revisions": env.revisions,
            }),
            Err(_) => json!({ "content": "", "content_html": "", "redirect": Value::Null, "revisions": [] }),
        };
        let categories = state
            .db
            .fetch_all_as::<CategoryRow>(
                "SELECT category_title AS title, category_slug AS slug FROM wiki.page_categories WHERE page_id = $1",
                params![r.id],
            )
            .await?;
        let categories: Vec<Value> = categories
            .into_iter()
            .map(|c| json!({ "title": c.title, "slug": c.slug }))
            .collect();
        changes.push(json!({
            "uuid": r.id, "kind": "modified", "change_seq": r.change_seq,
            "page": page, "content": content, "categories": categories,
        }));
    }
    Ok(Json(json!({ "changes": changes, "cursor": new_cursor, "has_more": has_more })))
}
