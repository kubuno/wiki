//! Sync deltas for the local-first pull (wikis / pages). Owner-scoped changes
//! past `cursor` (monotonic change_seq). Wiki changes inline their `members`;
//! page changes inline the `.kbwik` envelope (source + cached HTML + revisions)
//! and their category rows, so pages read offline. Pages are soft-deleted
//! (is_deleted flag) → no page tombstones; only wikis tombstone on hard delete.
//!
//! Scope: personal wikis (owner = requester) and their pages. Shared-wiki sync
//! is a follow-up.

use axum::{
    extract::{Query, State},
    Extension, Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::errors::Result;
use crate::middleware::WikiUser;
use crate::services::content_files;
use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct DeltaQuery {
    #[serde(default)]
    cursor: i64,
    limit: Option<i64>,
}

/// GET /wikis/delta — the requester's wikis + inline members, with tombstones.
pub async fn wikis_delta(
    State(state): State<AppState>,
    Extension(user): Extension<WikiUser>,
    Query(q): Query<DeltaQuery>,
) -> Result<Json<Value>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let rows: Vec<(Uuid, i64, String)> = sqlx::query_as(
        r#"SELECT id, change_seq, 'live' AS src FROM wikis WHERE owner_id=$1 AND change_seq>$2
           UNION ALL
           SELECT id, change_seq, 'tomb' AS src FROM wiki_tombstones WHERE owner_id=$1 AND change_seq>$2
           ORDER BY change_seq LIMIT $3"#,
    )
    .bind(user.id)
    .bind(q.cursor)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() as i64 == limit;
    let new_cursor = rows.last().map(|r| r.1).unwrap_or(q.cursor);
    let mut changes = Vec::with_capacity(rows.len());
    for (id, seq, src) in &rows {
        if src == "tomb" {
            changes.push(json!({ "uuid": id, "kind": "deleted", "change_seq": seq }));
            continue;
        }
        let wiki: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(w) FROM (SELECT id, owner_id, storage_owner_id, slug, name, description, \
             is_shared, created_at, updated_at FROM wikis WHERE id=$1) w",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
        let Some(wiki) = wiki else { continue };
        let members: Vec<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(m) FROM (SELECT user_id, role, added_at FROM wiki_members WHERE wiki_id=$1) m",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?;
        changes.push(json!({ "uuid": id, "kind": "modified", "change_seq": seq, "wiki": wiki, "members": members }));
    }
    Ok(Json(json!({ "changes": changes, "cursor": new_cursor, "has_more": has_more })))
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
    let rows: Vec<(Uuid, i64, Uuid)> = sqlx::query_as(
        r#"SELECT p.id, p.change_seq, w.storage_owner_id
           FROM pages p JOIN wikis w ON w.id = p.wiki_id
           WHERE w.owner_id = $1 AND p.change_seq > $2
           ORDER BY p.change_seq LIMIT $3"#,
    )
    .bind(user.id)
    .bind(q.cursor)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() as i64 == limit;
    let new_cursor = rows.last().map(|r| r.1).unwrap_or(q.cursor);
    let mut changes = Vec::with_capacity(rows.len());
    for (id, seq, storage_owner) in &rows {
        let page: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(p) FROM (SELECT id, wiki_id, namespace, title, slug, file_id, redirect_to, \
             preview, byte_size, current_author_id, current_rev_at, is_deleted, created_at, updated_at \
             FROM pages WHERE id=$1) p",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
        let Some(page) = page else { continue };
        let file_id: Uuid = page.get("file_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or_default();
        // Read the .kbwik envelope (source + cached HTML + revisions). Best-effort:
        // a missing file yields an empty envelope rather than failing the page.
        let content = match content_files::read_page_file(&state, *storage_owner, file_id).await {
            Ok(env) => json!({
                "version": env.version, "namespace": env.namespace, "title": env.title,
                "content": env.content, "content_html": env.content_html,
                "redirect": env.redirect, "revisions": env.revisions,
            }),
            Err(_) => json!({ "content": "", "content_html": "", "redirect": Value::Null, "revisions": [] }),
        };
        let categories: Vec<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(c) FROM (SELECT category_title AS title, category_slug AS slug \
             FROM page_categories WHERE page_id=$1) c",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?;
        changes.push(json!({
            "uuid": id, "kind": "modified", "change_seq": seq,
            "page": page, "content": content, "categories": categories,
        }));
    }
    Ok(Json(json!({ "changes": changes, "cursor": new_cursor, "has_more": has_more })))
}
