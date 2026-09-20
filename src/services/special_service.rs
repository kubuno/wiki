//! Special pages: all pages, recent changes, wanted/orphaned pages, categories.

use kubuno_db::params;
use serde::Serialize;
use uuid::Uuid;

use crate::errors::Result;
use crate::models::page::PageSummary;
use crate::state::AppState;

pub async fn all_pages(state: &AppState, wiki_id: Uuid, namespace: Option<&str>) -> Result<Vec<PageSummary>> {
    // The optional namespace filter is composed in Rust rather than with a
    // PostgreSQL-only `$2::text IS NULL` cast, so it is portable.
    let base = "SELECT id, namespace, title, slug, redirect_to, preview, byte_size, current_rev_at \
                FROM wiki.pages WHERE wiki_id = $1 AND NOT is_deleted";
    let rows = match namespace {
        Some(ns) => {
            let sql = format!("{base} AND namespace = $2 ORDER BY namespace, title");
            state.db.fetch_all_as::<PageSummary>(&sql, params![wiki_id, ns]).await?
        }
        None => {
            let sql = format!("{base} ORDER BY namespace, title");
            state.db.fetch_all_as::<PageSummary>(&sql, params![wiki_id]).await?
        }
    };
    Ok(rows)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RecentChange {
    pub id:          Uuid,
    pub page_id:     Option<Uuid>,
    pub namespace:   String,
    pub title:       String,
    pub author_id:   Option<Uuid>,
    pub author_name: String,
    pub comment:     String,
    pub minor:       bool,
    pub change_type: String,
    pub byte_delta:  i32,
    pub created_at:  chrono::DateTime<chrono::Utc>,
}

pub async fn recent_changes(state: &AppState, wiki_id: Uuid, limit: i64) -> Result<Vec<RecentChange>> {
    let rows = state
        .db
        .fetch_all_as::<RecentChange>(
            "SELECT id, page_id, namespace, title, author_id, author_name, comment, minor, change_type, byte_delta, created_at \
             FROM wiki.recent_changes WHERE wiki_id = $1 ORDER BY created_at DESC LIMIT $2",
            params![wiki_id, limit.clamp(1, 500)],
        )
        .await?;
    Ok(rows)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WantedPage {
    pub namespace: String,
    pub title:     String,
    pub slug:      String,
    pub refs:      i64,
}

/// Linked-but-missing pages, most-referenced first.
pub async fn wanted_pages(state: &AppState, wiki_id: Uuid) -> Result<Vec<WantedPage>> {
    let count = state.db.backend().count_bigint("*");
    let sql = format!(
        "SELECT target_namespace AS namespace, MIN(target_title) AS title, target_slug AS slug, {count} AS refs \
         FROM wiki.page_links \
         WHERE wiki_id = $1 AND target_page_id IS NULL \
         GROUP BY target_namespace, target_slug \
         ORDER BY refs DESC, title"
    );
    let rows = state.db.fetch_all_as::<WantedPage>(&sql, params![wiki_id]).await?;
    Ok(rows)
}

/// Content pages with no incoming links and that are not redirects.
pub async fn orphaned_pages(state: &AppState, wiki_id: Uuid) -> Result<Vec<PageSummary>> {
    let rows = state
        .db
        .fetch_all_as::<PageSummary>(
            "SELECT p.id, p.namespace, p.title, p.slug, p.redirect_to, p.preview, p.byte_size, p.current_rev_at \
             FROM wiki.pages p \
             WHERE p.wiki_id = $1 AND NOT p.is_deleted AND p.redirect_to IS NULL \
               AND p.namespace = 'Main' \
               AND NOT EXISTS (SELECT 1 FROM wiki.page_links l WHERE l.target_page_id = p.id) \
             ORDER BY p.title",
            params![wiki_id],
        )
        .await?;
    Ok(rows)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CategoryCount {
    pub title: String,
    pub slug:  String,
    pub pages: i64,
}

pub async fn categories(state: &AppState, wiki_id: Uuid) -> Result<Vec<CategoryCount>> {
    let count = state.db.backend().count_bigint("*");
    let sql = format!(
        "SELECT MIN(category_title) AS title, category_slug AS slug, {count} AS pages \
         FROM wiki.page_categories WHERE wiki_id = $1 \
         GROUP BY category_slug ORDER BY title"
    );
    let rows = state.db.fetch_all_as::<CategoryCount>(&sql, params![wiki_id]).await?;
    Ok(rows)
}

pub async fn category_members(state: &AppState, wiki_id: Uuid, category_slug: &str) -> Result<Vec<PageSummary>> {
    let rows = state
        .db
        .fetch_all_as::<PageSummary>(
            "SELECT p.id, p.namespace, p.title, p.slug, p.redirect_to, p.preview, p.byte_size, p.current_rev_at \
             FROM wiki.page_categories c JOIN wiki.pages p ON p.id = c.page_id \
             WHERE c.wiki_id = $1 AND c.category_slug = $2 AND NOT p.is_deleted \
             ORDER BY p.namespace, p.title",
            params![wiki_id, category_slug],
        )
        .await?;
    Ok(rows)
}
