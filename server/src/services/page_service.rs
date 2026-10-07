//! Page lifecycle: render, save (new revision), move, delete, history.
//!
//! The page source / HTML / revision history live in the `.kbwik` file; the
//! `pages` table is just an index. The link graph (`page_links`), category
//! membership (`page_categories`) and the `recent_changes` feed are recomputed
//! on every save inside a single transaction.
//!
//! Delta: pages are versioned (a `change_seq` per row) and SOFT-deleted (the
//! `is_deleted` flag), so a deletion is just a `modified` change and pages never
//! tombstone. Every insert / update / soft-delete / move takes a fresh page seq
//! from `crate::sync` inside its transaction.
//!
//! Search: `title_norm` / `body_norm` hold the Snowball-French-stemmed,
//! deaccented forms of the title and the preview, computed in Rust with
//! `kubuno_db::search::normalize` at every write (the portable replacement for
//! the old `setweight(to_tsvector('french', unaccent(...)))` trigger).

use chrono::Utc;
use kubuno_db::search::normalize;
use kubuno_db::{new_id, params};
use uuid::Uuid;

use crate::errors::{Result, WikiError};
use crate::models::page::{self, Page, PageSummary, SavePageRequest};
use crate::models::wiki::Wiki;
use crate::services::content_files::{self, PageEnvelope, Revision};
use crate::services::wiki_markup::{self, RenderResult};
use crate::state::AppState;
use crate::sync;

pub struct RenderedPage {
    pub page:   Page,
    pub source: String,
    pub render: RenderResult,
}

/// Applies the instance's revision-retention window to a page's history.
///
/// Revisions are appended, so the head of the vector is the oldest one — that is
/// what a retention window drops. `keep == 0` means "keep everything", which is
/// the default: forgetting history must be an explicit decision, never one an
/// administrator makes by leaving a field alone.
///
/// The newest revision is never dropped: it is the page's current content, and
/// losing it would leave the file describing a state nothing points at.
fn trim_revisions(revisions: &mut Vec<Revision>, keep: u32) {
    if keep == 0 {
        return; // retention disabled: the whole history stays
    }
    let keep = keep as usize;
    if revisions.len() > keep {
        revisions.drain(..revisions.len() - keep);
    }
}

pub async fn list_pages(state: &AppState, wiki_id: Uuid) -> Result<Vec<PageSummary>> {
    let rows = state
        .db
        .fetch_all_as::<PageSummary>(
            "SELECT id, namespace, title, slug, redirect_to, preview, byte_size, current_rev_at \
             FROM wiki.pages WHERE wiki_id = $1 AND NOT is_deleted \
             ORDER BY namespace, title",
            params![wiki_id],
        )
        .await?;
    Ok(rows)
}

async fn find_page(state: &AppState, wiki_id: Uuid, ns: &str, slug: &str) -> Result<Option<Page>> {
    let row = state
        .db
        .fetch_optional_as::<Page>(
            "SELECT * FROM wiki.pages WHERE wiki_id = $1 AND namespace = $2 AND slug = $3 AND NOT is_deleted",
            params![wiki_id, ns, slug],
        )
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
    if req.content.len() as u64 > state.instance().max_content_size {
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
    // Normalized search columns (title = weight A, preview = weight B).
    let title_norm = normalize(&title);
    let body_norm = normalize(&preview);

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
            trim_revisions(&mut env.revisions, state.instance().max_revisions_per_page);
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
    let seq = sync::next_page_seq(&mut tx).await?;

    let page_id: Uuid = if let Some(p) = &existing {
        tx.execute(
            "UPDATE wiki.pages SET title=$1, redirect_to=$2, preview=$3, byte_size=$4, \
                current_author_id=$5, current_rev_at=$6, title_norm=$7, body_norm=$8, change_seq=$9 \
             WHERE id=$10",
            params![
                &title,
                render.redirect.as_deref(),
                &preview,
                byte_size,
                author_id,
                now,
                &title_norm,
                &body_norm,
                seq,
                p.id
            ],
        )
        .await?;
        p.id
    } else {
        let id = req.id.unwrap_or_else(new_id);
        tx.execute(
            "INSERT INTO wiki.pages (id, wiki_id, namespace, title, slug, file_id, redirect_to, preview, \
                byte_size, current_author_id, current_rev_at, title_norm, body_norm, change_seq) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
            params![
                id,
                wiki.id,
                &ns,
                &title,
                &slug,
                file_id,
                render.redirect.as_deref(),
                &preview,
                byte_size,
                author_id,
                now,
                &title_norm,
                &body_norm,
                seq
            ],
        )
        .await?;

        // Resolve pending red links that pointed at this new page.
        tx.execute(
            "UPDATE wiki.page_links SET target_page_id = $1 \
             WHERE wiki_id = $2 AND target_namespace = $3 AND target_slug = $4 AND target_page_id IS NULL",
            params![id, wiki.id, &ns, &slug],
        )
        .await?;
        id
    };

    // Rebuild outgoing links.
    let link_ignore = state.db.backend().insert_ignore_prefix();
    let link_nothing = state
        .db
        .backend()
        .on_conflict_do_nothing(&["source_page_id", "target_namespace", "target_slug"]);
    tx.execute("DELETE FROM wiki.page_links WHERE source_page_id = $1", params![page_id])
        .await?;
    for link in &render.links {
        if link.namespace == ns && link.slug == slug {
            continue; // ignore self-links
        }
        let target_id: Option<Uuid> = tx
            .fetch_optional_scalar::<Uuid>(
                "SELECT id FROM wiki.pages WHERE wiki_id=$1 AND namespace=$2 AND slug=$3 AND NOT is_deleted",
                params![wiki.id, &link.namespace, &link.slug],
            )
            .await?;
        tx.execute(
            &format!(
                "INSERT {link_ignore}INTO wiki.page_links \
                    (source_page_id, wiki_id, target_namespace, target_title, target_slug, target_page_id) \
                 VALUES ($1,$2,$3,$4,$5,$6){link_nothing}"
            ),
            params![page_id, wiki.id, &link.namespace, &link.title, &link.slug, target_id],
        )
        .await?;
    }

    // Rebuild categories.
    let cat_ignore = state.db.backend().insert_ignore_prefix();
    let cat_nothing = state
        .db
        .backend()
        .on_conflict_do_nothing(&["page_id", "category_slug"]);
    tx.execute("DELETE FROM wiki.page_categories WHERE page_id = $1", params![page_id])
        .await?;
    for cat in &render.categories {
        tx.execute(
            &format!(
                "INSERT {cat_ignore}INTO wiki.page_categories (page_id, wiki_id, category_title, category_slug) \
                 VALUES ($1,$2,$3,$4){cat_nothing}"
            ),
            params![page_id, wiki.id, &cat.title, &cat.slug],
        )
        .await?;
    }

    // Recent changes entry.
    let byte_delta = byte_size - existing.as_ref().map(|p| p.byte_size).unwrap_or(0);
    tx.execute(
        "INSERT INTO wiki.recent_changes \
            (id, wiki_id, page_id, namespace, title, author_id, author_name, comment, minor, change_type, byte_delta) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        params![
            new_id(),
            wiki.id,
            page_id,
            &ns,
            &title,
            author_id,
            author_name,
            &req.comment,
            req.minor,
            change_type,
            byte_delta
        ],
    )
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
    let page = state
        .db
        .fetch_optional_as::<Page>(
            "SELECT * FROM wiki.pages WHERE id = $1 AND wiki_id = $2",
            params![page_id, wiki.id],
        )
        .await?
        .ok_or_else(|| WikiError::NotFound("page".into()))?;

    let mut tx = state.db.begin().await?;
    let seq = sync::next_page_seq(&mut tx).await?;
    // Soft-delete: a `modified` change carrying is_deleted=true (no tombstone).
    tx.execute(
        "UPDATE wiki.pages SET is_deleted = $1, change_seq = $2 WHERE id = $3",
        params![true, seq, page_id],
    )
    .await?;
    // Orphan inbound links (they become red links again).
    tx.execute(
        "UPDATE wiki.page_links SET target_page_id = NULL WHERE target_page_id = $1",
        params![page_id],
    )
    .await?;
    tx.execute("DELETE FROM wiki.page_links WHERE source_page_id = $1", params![page_id])
        .await?;
    tx.execute("DELETE FROM wiki.page_categories WHERE page_id = $1", params![page_id])
        .await?;
    tx.execute(
        "INSERT INTO wiki.recent_changes \
            (id, wiki_id, page_id, namespace, title, author_id, author_name, comment, minor, change_type, byte_delta) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        params![
            new_id(),
            wiki.id,
            page_id,
            &page.namespace,
            &page.title,
            author_id,
            "",
            "",
            false,
            "delete",
            0i32
        ],
    )
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
    let page = state
        .db
        .fetch_optional_as::<Page>(
            "SELECT * FROM wiki.pages WHERE id = $1 AND wiki_id = $2 AND NOT is_deleted",
            params![page_id, wiki.id],
        )
        .await?
        .ok_or_else(|| WikiError::NotFound("page".into()))?;

    let (new_ns, new_title) = page::split_namespace(new_ref);
    let new_slug = page::slugify(&new_title);

    if find_page(state, wiki.id, &new_ns, &new_slug).await?.is_some() {
        return Err(WikiError::Conflict("target title already exists".into()));
    }

    let title_norm = normalize(&new_title);
    let mut tx = state.db.begin().await?;
    let seq = sync::next_page_seq(&mut tx).await?;
    tx.execute(
        "UPDATE wiki.pages SET namespace=$1, title=$2, slug=$3, title_norm=$4, change_seq=$5 WHERE id=$6",
        params![&new_ns, &new_title, &new_slug, &title_norm, seq, page_id],
    )
    .await?;
    tx.execute(
        "INSERT INTO wiki.recent_changes \
            (id, wiki_id, page_id, namespace, title, author_id, author_name, comment, minor, change_type, byte_delta) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        params![
            new_id(),
            wiki.id,
            page_id,
            &new_ns,
            &new_title,
            author_id,
            "",
            "",
            false,
            "move",
            0i32
        ],
    )
    .await?;
    tx.commit().await?;

    // Update the stored envelope title and rename the underlying file.
    if let Ok(mut env) = content_files::read_page_file(state, wiki.storage_owner_id, page.file_id).await {
        env.namespace = new_ns.clone();
        env.title = new_title.clone();
        let _ = content_files::write_page_file(state, wiki.storage_owner_id, page.file_id, &env).await;
    }
    content_files::rename_page_file(state, wiki.storage_owner_id, page.file_id, &new_title).await;

    find_page(state, wiki.id, &new_ns, &new_slug)
        .await?
        .ok_or_else(|| WikiError::Internal(anyhow::anyhow!("page vanished after move")))
}

// ── History (read from the .kbwik file) ─────────────────────────────────────

pub async fn history(state: &AppState, wiki: &Wiki, page_id: Uuid) -> Result<Vec<serde_json::Value>> {
    let page = state
        .db
        .fetch_optional_as::<Page>(
            "SELECT * FROM wiki.pages WHERE id = $1 AND wiki_id = $2",
            params![page_id, wiki.id],
        )
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
    let page = state
        .db
        .fetch_optional_as::<Page>(
            "SELECT * FROM wiki.pages WHERE id = $1 AND wiki_id = $2",
            params![page_id, wiki.id],
        )
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
    // `user_id` appears three times; bound once per occurrence for portability.
    let rows = state
        .db
        .fetch_all_as::<RecentPage>(
            "SELECT p.wiki_id, p.namespace, p.title, p.slug, p.current_rev_at \
             FROM wiki.pages p \
             JOIN wiki.wikis w ON w.id = p.wiki_id \
             LEFT JOIN wiki.wiki_members m ON m.wiki_id = w.id AND m.user_id = $1 \
             WHERE NOT p.is_deleted AND (w.owner_id = $2 OR m.user_id = $3) \
             ORDER BY p.current_rev_at DESC \
             LIMIT $4",
            params![user_id, user_id, user_id, limit.clamp(1, 50)],
        )
        .await?;
    Ok(rows)
}

/// Resolves a page by its underlying `.kbwik` file id (FileTypeRegistry "open").
pub async fn locate_by_file(state: &AppState, file_id: Uuid) -> Result<(Uuid, String, String)> {
    let row = state
        .db
        .fetch_optional_as::<(Uuid, String, String)>(
            "SELECT wiki_id, namespace, title FROM wiki.pages WHERE file_id = $1 AND NOT is_deleted",
            params![file_id],
        )
        .await?
        .ok_or_else(|| WikiError::NotFound("page".into()))?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rev(n: u8) -> Revision {
        Revision {
            rev_id:      Uuid::from_u128(n as u128),
            author_id:   None,
            author_name: String::new(),
            ts:          String::new(),
            comment:     String::new(),
            minor:       false,
            content:     String::new(),
            size:        0,
        }
    }

    #[test]
    fn zero_keeps_the_whole_history() {
        let mut revs: Vec<Revision> = (1..=5).map(rev).collect();
        trim_revisions(&mut revs, 0);
        assert_eq!(revs.len(), 5);
    }

    #[test]
    fn the_oldest_revisions_are_the_ones_dropped() {
        let mut revs: Vec<Revision> = (1..=5).map(rev).collect();
        trim_revisions(&mut revs, 2);
        assert_eq!(revs.len(), 2);
        // 4 and 5 survive — the current content is always the last one.
        assert_eq!(revs[0].rev_id, Uuid::from_u128(4));
        assert_eq!(revs[1].rev_id, Uuid::from_u128(5));
    }

    #[test]
    fn a_shorter_history_than_the_window_is_untouched() {
        let mut revs: Vec<Revision> = (1..=2).map(rev).collect();
        trim_revisions(&mut revs, 10);
        assert_eq!(revs.len(), 2);
    }
}
