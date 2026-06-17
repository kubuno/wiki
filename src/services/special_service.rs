//! Special pages: all pages, recent changes, wanted/orphaned pages, categories.

use serde::Serialize;
use uuid::Uuid;

use crate::errors::Result;
use crate::models::page::PageSummary;
use crate::state::AppState;

pub async fn all_pages(state: &AppState, wiki_id: Uuid, namespace: Option<&str>) -> Result<Vec<PageSummary>> {
    let rows = sqlx::query_as::<_, PageSummary>(
        "SELECT id, namespace, title, slug, redirect_to, preview, byte_size, current_rev_at \
         FROM pages \
         WHERE wiki_id = $1 AND NOT is_deleted AND ($2::text IS NULL OR namespace = $2) \
         ORDER BY namespace, title",
    )
    .bind(wiki_id)
    .bind(namespace)
    .fetch_all(&state.db)
    .await?;
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
    let rows = sqlx::query_as::<_, RecentChange>(
        "SELECT id, page_id, namespace, title, author_id, author_name, comment, minor, change_type, byte_delta, created_at \
         FROM recent_changes WHERE wiki_id = $1 ORDER BY created_at DESC LIMIT $2",
    )
    .bind(wiki_id)
    .bind(limit.clamp(1, 500))
    .fetch_all(&state.db)
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
    let rows = sqlx::query_as::<_, WantedPage>(
        "SELECT target_namespace AS namespace, MIN(target_title) AS title, target_slug AS slug, COUNT(*) AS refs \
         FROM page_links \
         WHERE wiki_id = $1 AND target_page_id IS NULL \
         GROUP BY target_namespace, target_slug \
         ORDER BY refs DESC, title",
    )
    .bind(wiki_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

/// Content pages with no incoming links and that are not redirects.
pub async fn orphaned_pages(state: &AppState, wiki_id: Uuid) -> Result<Vec<PageSummary>> {
    let rows = sqlx::query_as::<_, PageSummary>(
        "SELECT p.id, p.namespace, p.title, p.slug, p.redirect_to, p.preview, p.byte_size, p.current_rev_at \
         FROM pages p \
         WHERE p.wiki_id = $1 AND NOT p.is_deleted AND p.redirect_to IS NULL \
           AND p.namespace = 'Main' \
           AND NOT EXISTS (SELECT 1 FROM page_links l WHERE l.target_page_id = p.id) \
         ORDER BY p.title",
    )
    .bind(wiki_id)
    .fetch_all(&state.db)
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
    let rows = sqlx::query_as::<_, CategoryCount>(
        "SELECT MIN(category_title) AS title, category_slug AS slug, COUNT(*) AS pages \
         FROM page_categories WHERE wiki_id = $1 \
         GROUP BY category_slug ORDER BY title",
    )
    .bind(wiki_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

pub async fn category_members(state: &AppState, wiki_id: Uuid, category_slug: &str) -> Result<Vec<PageSummary>> {
    let rows = sqlx::query_as::<_, PageSummary>(
        "SELECT p.id, p.namespace, p.title, p.slug, p.redirect_to, p.preview, p.byte_size, p.current_rev_at \
         FROM page_categories c JOIN pages p ON p.id = c.page_id \
         WHERE c.wiki_id = $1 AND c.category_slug = $2 AND NOT p.is_deleted \
         ORDER BY p.namespace, p.title",
    )
    .bind(wiki_id)
    .bind(category_slug)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}
