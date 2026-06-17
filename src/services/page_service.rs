//! Page lifecycle: render, save (new revision), move, delete, history.
//!
//! The page source / HTML / revision history live in the `.kbwik` file; the
//! `pages` table is just an index. The link graph (`page_links`), category
//! membership (`page_categories`) and the `recent_changes` feed are recomputed
//! on every save inside a single transaction.

use chrono::Utc;
use uuid::Uuid;

use crate::errors::{Result, WikiError};
use crate::models::page::{self, Page, PageSummary, SavePageRequest};
use crate::models::wiki::Wiki;
use crate::services::content_files::{self, PageEnvelope, Revision};
use crate::services::wiki_markup::{self, RenderResult};
use crate::state::AppState;

pub struct RenderedPage {
    pub page:   Page,
    pub source: String,
    pub render: RenderResult,
}

pub async fn list_pages(state: &AppState, wiki_id: Uuid) -> Result<Vec<PageSummary>> {
    let rows = sqlx::query_as::<_, PageSummary>(
        "SELECT id, namespace, title, slug, redirect_to, preview, byte_size, current_rev_at \
         FROM pages WHERE wiki_id = $1 AND NOT is_deleted \
         ORDER BY namespace, title",
    )
    .bind(wiki_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

async fn find_page(state: &AppState, wiki_id: Uuid, ns: &str, slug: &str) -> Result<Option<Page>> {
    let row = sqlx::query_as::<_, Page>(
        "SELECT * FROM pages WHERE wiki_id = $1 AND namespace = $2 AND slug = $3 AND NOT is_deleted",
    )
    .bind(wiki_id)
    .bind(ns)
    .bind(slug)
    .fetch_optional(&state.db)
    .await?;
    Ok(row)
}

/// Loads and renders a page. Returns `None` when the page does not exist
/// (callers then offer to create it).
pub async fn get_rendered(
    state: &AppState,
    wiki: &Wiki,
    ns: &str,
    title: &str,
) -> Result<Option<RenderedPage>> {
    let slug = page::slugify(title);
    let Some(page) = find_page(state, wiki.id, ns, &slug).await? else {
        return Ok(None);
    };
    let env = content_files::read_page_file(state, wiki.storage_owner_id, page.file_id).await?;
    let render = wiki_markup::render_page(state, wiki.id, ns, &page.title, &env.content).await?;
    Ok(Some(RenderedPage { page, source: env.content, render }))
}

/// Preview rendering without persistence.
pub async fn preview(state: &AppState, wiki: &Wiki, ns: &str, title: &str, source: &str) -> Result<RenderResult> {
    wiki_markup::render_page(state, wiki.id, ns, title, source).await
}

/// Creates or updates a page (appends a revision).
pub async fn save_page(
    state: &AppState,
    wiki: &Wiki,
    author_id: Uuid,
    author_name: &str,
    req: SavePageRequest,
) -> Result<Page> {
    if req.content.len() as u64 > state.settings.wiki.max_content_size {
        return Err(WikiError::ContentTooLarge);
    }

    let ns = req
        .namespace
        .as_deref()
        .and_then(page::canonical_namespace)
        .unwrap_or("Main")
        .to_string();
    let title = page::normalize_title(&req.title);
    if title.is_empty() {
        return Err(WikiError::Validation("title is required".into()));
    }
    let slug = page::slugify(&title);

    // Render (resolves links/templates against the current index).
    let render = wiki_markup::render_page(state, wiki.id, &ns, &title, &req.content).await?;
    let preview = content_files::make_preview(&req.content);
    let byte_size = req.content.len() as i32;
    let now = Utc::now();

    let existing = find_page(state, wiki.id, &ns, &slug).await?;

    // ── Update the .kbwik file (outside the DB transaction). ──
    let new_rev = Revision {
        rev_id:      Uuid::new_v4(),
        author_id:   Some(author_id),
        author_name: author_name.to_string(),
        ts:          now.to_rfc3339(),
        comment:     req.comment.clone(),
        minor:       req.minor,
        content:     req.content.clone(),
        size:        byte_size as i64,
    };

    let (file_id, change_type) = match &existing {
        Some(p) => {
            let mut env = content_files::read_page_file(state, wiki.storage_owner_id, p.file_id).await?;
            env.content = req.content.clone();
            env.content_html = render.html.clone();
            env.redirect = render.redirect.clone();
            env.revisions.push(new_rev);
            content_files::write_page_file(state, wiki.storage_owner_id, p.file_id, &env).await?;
            (p.file_id, "edit")
        }
        None => {
            let env = PageEnvelope {
                version:      1,
                namespace:    ns.clone(),
                title:        title.clone(),
                content:      req.content.clone(),
                content_html: render.html.clone(),
                redirect:     render.redirect.clone(),
                revisions:    vec![new_rev],
            };
            let file_id = content_files::create_page_file(state, wiki.storage_owner_id, &wiki.slug, &env).await?;
            (file_id, "create")
        }
    };

    // ── Index transaction. ──
    let mut tx = state.db.begin().await?;

    let page_id: Uuid = if let Some(p) = &existing {
        sqlx::query(
            "UPDATE pages SET title=$2, redirect_to=$3, preview=$4, byte_size=$5, \
                current_author_id=$6, current_rev_at=$7 WHERE id=$1",
        )
        .bind(p.id)
        .bind(&title)
        .bind(&render.redirect)
        .bind(&preview)
        .bind(byte_size)
        .bind(author_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        p.id
    } else {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO pages (wiki_id, namespace, title, slug, file_id, redirect_to, preview, \
                byte_size, current_author_id, current_rev_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING id",
        )
        .bind(wiki.id)
        .bind(&ns)
        .bind(&title)
        .bind(&slug)
        .bind(file_id)
        .bind(&render.redirect)
        .bind(&preview)
        .bind(byte_size)
        .bind(author_id)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;

        // Resolve pending red links that pointed at this new page.
        sqlx::query(
            "UPDATE page_links SET target_page_id = $1 \
             WHERE wiki_id = $2 AND target_namespace = $3 AND target_slug = $4 AND target_page_id IS NULL",
        )
        .bind(id)
        .bind(wiki.id)
        .bind(&ns)
        .bind(&slug)
        .execute(&mut *tx)
        .await?;
        id
    };

    // Rebuild outgoing links.
    sqlx::query("DELETE FROM page_links WHERE source_page_id = $1")
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
    for link in &render.links {
        if link.namespace == ns && link.slug == slug {
            continue; // ignore self-links
        }
        let target_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM pages WHERE wiki_id=$1 AND namespace=$2 AND slug=$3 AND NOT is_deleted",
        )
        .bind(wiki.id)
        .bind(&link.namespace)
        .bind(&link.slug)
        .fetch_optional(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO page_links (source_page_id, wiki_id, target_namespace, target_title, target_slug, target_page_id) \
             VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
        )
        .bind(page_id)
        .bind(wiki.id)
        .bind(&link.namespace)
        .bind(&link.title)
        .bind(&link.slug)
        .bind(target_id)
        .execute(&mut *tx)
        .await?;
    }

    // Rebuild categories.
    sqlx::query("DELETE FROM page_categories WHERE page_id = $1")
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
    for cat in &render.categories {
        sqlx::query(
            "INSERT INTO page_categories (page_id, wiki_id, category_title, category_slug) \
             VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING",
        )
        .bind(page_id)
        .bind(wiki.id)
        .bind(&cat.title)
        .bind(&cat.slug)
        .execute(&mut *tx)
        .await?;
    }

    // Recent changes entry.
    let byte_delta = byte_size - existing.as_ref().map(|p| p.byte_size).unwrap_or(0);
    sqlx::query(
        "INSERT INTO recent_changes (wiki_id, page_id, namespace, title, author_id, author_name, comment, minor, change_type, byte_delta) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(wiki.id)
    .bind(page_id)
    .bind(&ns)
    .bind(&title)
    .bind(author_id)
    .bind(author_name)
    .bind(&req.comment)
    .bind(req.minor)
    .bind(change_type)
    .bind(byte_delta)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Best-effort: keep the file name in sync with the title.
    content_files::rename_page_file(state, wiki.storage_owner_id, file_id, &title).await;

    let saved = find_page(state, wiki.id, &ns, &slug)
        .await?
        .ok_or_else(|| WikiError::Internal(anyhow::anyhow!("page vanished after save")))?;

    // Publish event.
    let event = if change_type == "create" {
        crate::events::page_created_event(wiki.id, page_id, author_id)
    } else {
        crate::events::page_updated_event(wiki.id, page_id, author_id)
    };
    state.publish(event).await;

    Ok(saved)
}

pub async fn delete_page(state: &AppState, wiki: &Wiki, author_id: Uuid, page_id: Uuid) -> Result<()> {
    let page = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = $1 AND wiki_id = $2")
        .bind(page_id)
        .bind(wiki.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| WikiError::NotFound("page".into()))?;

    let mut tx = state.db.begin().await?;
    // Mark deleted; orphan inbound links (they become red links again).
    sqlx::query("UPDATE pages SET is_deleted = TRUE WHERE id = $1")
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE page_links SET target_page_id = NULL WHERE target_page_id = $1")
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM page_links WHERE source_page_id = $1")
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM page_categories WHERE page_id = $1")
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO recent_changes (wiki_id, page_id, namespace, title, author_id, change_type) \
         VALUES ($1,$2,$3,$4,$5,'delete')",
    )
    .bind(wiki.id)
    .bind(page_id)
    .bind(&page.namespace)
    .bind(&page.title)
    .bind(author_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    content_files::delete_page_file(state, wiki.storage_owner_id, page.file_id).await;
    state.publish(crate::events::page_deleted_event(wiki.id, page_id, author_id)).await;
    Ok(())
}

pub async fn move_page(
    state: &AppState,
    wiki: &Wiki,
    author_id: Uuid,
    page_id: Uuid,
    new_ref: &str,
) -> Result<Page> {
    let page = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = $1 AND wiki_id = $2 AND NOT is_deleted")
        .bind(page_id)
        .bind(wiki.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| WikiError::NotFound("page".into()))?;

    let (new_ns, new_title) = page::split_namespace(new_ref);
    let new_slug = page::slugify(&new_title);

    if find_page(state, wiki.id, &new_ns, &new_slug).await?.is_some() {
        return Err(WikiError::Conflict("target title already exists".into()));
    }

    sqlx::query("UPDATE pages SET namespace=$2, title=$3, slug=$4 WHERE id=$1")
        .bind(page_id)
        .bind(&new_ns)
        .bind(&new_title)
        .bind(&new_slug)
        .execute(&state.db)
        .await?;

    // Update the stored envelope title and re-resolve who links here.
    if let Ok(mut env) = content_files::read_page_file(state, wiki.storage_owner_id, page.file_id).await {
        env.namespace = new_ns.clone();
        env.title = new_title.clone();
        let _ = content_files::write_page_file(state, wiki.storage_owner_id, page.file_id, &env).await;
    }
    content_files::rename_page_file(state, wiki.storage_owner_id, page.file_id, &new_title).await;

    sqlx::query(
        "INSERT INTO recent_changes (wiki_id, page_id, namespace, title, author_id, change_type) \
         VALUES ($1,$2,$3,$4,$5,'move')",
    )
    .bind(wiki.id)
    .bind(page_id)
    .bind(&new_ns)
    .bind(&new_title)
    .bind(author_id)
    .execute(&state.db)
    .await?;

    find_page(state, wiki.id, &new_ns, &new_slug)
        .await?
        .ok_or_else(|| WikiError::Internal(anyhow::anyhow!("page vanished after move")))
}

// ── History (read from the .kbwik file) ─────────────────────────────────────

pub async fn history(state: &AppState, wiki: &Wiki, page_id: Uuid) -> Result<Vec<serde_json::Value>> {
    let page = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = $1 AND wiki_id = $2")
        .bind(page_id)
        .bind(wiki.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| WikiError::NotFound("page".into()))?;
    let env = content_files::read_page_file(state, wiki.storage_owner_id, page.file_id).await?;
    let out = env
        .revisions
        .iter()
        .rev()
        .map(|r| {
            serde_json::json!({
                "rev_id": r.rev_id,
                "author_id": r.author_id,
                "author_name": r.author_name,
                "ts": r.ts,
                "comment": r.comment,
                "minor": r.minor,
                "size": r.size,
            })
        })
        .collect();
    Ok(out)
}

pub async fn revision_content(state: &AppState, wiki: &Wiki, page_id: Uuid, rev_id: Uuid) -> Result<Revision> {
    let page = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = $1 AND wiki_id = $2")
        .bind(page_id)
        .bind(wiki.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| WikiError::NotFound("page".into()))?;
    let env = content_files::read_page_file(state, wiki.storage_owner_id, page.file_id).await?;
    env.revisions
        .into_iter()
        .find(|r| r.rev_id == rev_id)
        .ok_or_else(|| WikiError::NotFound("revision".into()))
}

/// Recently edited pages across every wiki the user can access (launcher feed).
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct RecentPage {
    pub wiki_id:        Uuid,
    pub namespace:      String,
    pub title:          String,
    pub slug:           String,
    pub current_rev_at: chrono::DateTime<chrono::Utc>,
}

pub async fn recent_pages(state: &AppState, user_id: Uuid, limit: i64) -> Result<Vec<RecentPage>> {
    let rows = sqlx::query_as::<_, RecentPage>(
        "SELECT p.wiki_id, p.namespace, p.title, p.slug, p.current_rev_at \
         FROM pages p \
         JOIN wikis w ON w.id = p.wiki_id \
         LEFT JOIN wiki_members m ON m.wiki_id = w.id AND m.user_id = $1 \
         WHERE NOT p.is_deleted AND (w.owner_id = $1 OR m.user_id = $1) \
         ORDER BY p.current_rev_at DESC \
         LIMIT $2",
    )
    .bind(user_id)
    .bind(limit.clamp(1, 50))
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

/// Resolves a page by its underlying `.kbwik` file id (FileTypeRegistry "open").
pub async fn locate_by_file(state: &AppState, file_id: Uuid) -> Result<(Uuid, String, String)> {
    let row = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT wiki_id, namespace, title FROM pages WHERE file_id = $1 AND NOT is_deleted",
    )
    .bind(file_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| WikiError::NotFound("page".into()))?;
    Ok(row)
}
