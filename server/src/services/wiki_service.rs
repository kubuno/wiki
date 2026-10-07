//! Wiki space lifecycle: create / list / update / delete, plus membership
//! management for shared wikis. User identities are resolved against `core.users`.
//!
//! Delta: wikis are versioned (a `change_seq` per row) and hard-deletes write a
//! tombstone; members ride inline in the wiki delta, so a membership write bumps
//! its wiki. All of that goes through `crate::sync` / `kubuno_db::journal`.
//!
//! Reservation: the membership queries join `core.users` (a foreign namespace).
//! That cross-schema access resolves on PostgreSQL (and MySQL on the same
//! server) but not on an ATTACHed SQLite file; it is the account-directory
//! boundary, not something the portable recipe covers, and is left as-is.

use kubuno_db::dialect::Assign;
use kubuno_db::{new_id, params};
use uuid::Uuid;

use crate::errors::{Result, WikiError};
use crate::models::member::{Role, WikiMemberView};
use crate::models::wiki::{CreateWikiRequest, UpdateWikiRequest, Wiki, WikiView};
use crate::services::{content_files, permission_service};
use crate::state::AppState;
use crate::sync;

/// Reserved system user that owns the storage of shared wikis (same convention
/// as the shared System directory in core/drive).
pub fn system_owner() -> Uuid {
    Uuid::from_u128(1)
}

fn base_slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    let collapsed: String = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if collapsed.is_empty() { "wiki".to_string() } else { collapsed }
}

async fn unique_slug(state: &AppState, owner_id: Uuid, name: &str) -> Result<String> {
    let base = base_slug(name);
    for n in 0.. {
        let candidate = if n == 0 { base.clone() } else { format!("{base}-{n}") };
        let exists = state
            .db
            .fetch_optional_scalar::<Uuid>(
                "SELECT id FROM wiki.wikis WHERE owner_id = $1 AND slug = $2 LIMIT 1",
                params![owner_id, &candidate],
            )
            .await?
            .is_some();
        if !exists {
            return Ok(candidate);
        }
    }
    unreachable!()
}

pub async fn list_wikis(state: &AppState, user_id: Uuid) -> Result<Vec<WikiView>> {
    // `user_id` appears three times; portable SQL cannot reuse a placeholder, so
    // it is bound once per occurrence ($1 join, $2 owner, $3 member).
    let wikis = state
        .db
        .fetch_all_as::<Wiki>(
            "SELECT DISTINCT w.* FROM wiki.wikis w \
             LEFT JOIN wiki.wiki_members m ON m.wiki_id = w.id AND m.user_id = $1 \
             WHERE w.owner_id = $2 OR m.user_id = $3 \
             ORDER BY w.name",
            params![user_id, user_id, user_id],
        )
        .await?;

    let count_expr = state.db.backend().count_bigint("*");
    let mut out = Vec::with_capacity(wikis.len());
    for w in wikis {
        let role = permission_service::effective_role(state, &w, user_id).await?;
        let page_count: i64 = state
            .db
            .fetch_scalar::<i64>(
                &format!("SELECT {count_expr} FROM wiki.pages WHERE wiki_id = $1 AND NOT is_deleted"),
                params![w.id],
            )
            .await?;
        out.push(WikiView { wiki: w, my_role: role.as_str().to_string(), page_count });
    }
    Ok(out)
}

pub async fn create_wiki(state: &AppState, user_id: Uuid, req: CreateWikiRequest) -> Result<Wiki> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(WikiError::Validation("name is required".into()));
    }
    let slug = unique_slug(state, user_id, name).await?;
    let storage_owner = if req.is_shared { system_owner() } else { user_id };
    let id = req.id.unwrap_or_else(new_id);

    // Insert the row and its first change_seq atomically, then reselect on the
    // pool (a DbTx has no typed fetch, and MySQL has no RETURNING).
    let mut tx = state.db.begin().await?;
    let seq = sync::next_wiki_seq(&mut tx).await?;
    tx.execute(
        "INSERT INTO wiki.wikis (id, owner_id, storage_owner_id, slug, name, description, is_shared, change_seq) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        params![id, user_id, storage_owner, &slug, name, req.description.trim(), req.is_shared, seq],
    )
    .await?;
    tx.commit().await?;

    let wiki = state
        .db
        .fetch_one_as::<Wiki>("SELECT * FROM wiki.wikis WHERE id = $1", params![id])
        .await?;

    // Pre-create the storage folder (best-effort).
    let _ = state
        .files_client
        .ensure_folder_path(storage_owner, &format!("Wiki/{slug}"), true, Some("BookMarked"))
        .await;

    Ok(wiki)
}

pub async fn get_wiki_view(state: &AppState, wiki_id: Uuid, user_id: Uuid) -> Result<WikiView> {
    let (wiki, role) = permission_service::authorize(state, wiki_id, user_id).await?;
    let count_expr = state.db.backend().count_bigint("*");
    let page_count: i64 = state
        .db
        .fetch_scalar::<i64>(
            &format!("SELECT {count_expr} FROM wiki.pages WHERE wiki_id = $1 AND NOT is_deleted"),
            params![wiki_id],
        )
        .await?;
    Ok(WikiView { wiki, my_role: role.as_str().to_string(), page_count })
}

pub async fn update_wiki(
    state: &AppState,
    wiki_id: Uuid,
    user_id: Uuid,
    req: UpdateWikiRequest,
) -> Result<Wiki> {
    permission_service::require_admin(state, wiki_id, user_id).await?;
    let mut tx = state.db.begin().await?;
    let seq = sync::next_wiki_seq(&mut tx).await?;
    tx.execute(
        "UPDATE wiki.wikis SET \
            name        = COALESCE($1, name), \
            description = COALESCE($2, description), \
            change_seq  = $3 \
         WHERE id = $4",
        params![
            req.name.as_deref().map(str::trim),
            req.description.as_deref().map(str::trim),
            seq,
            wiki_id
        ],
    )
    .await?;
    tx.commit().await?;

    state
        .db
        .fetch_one_as::<Wiki>("SELECT * FROM wiki.wikis WHERE id = $1", params![wiki_id])
        .await
        .map_err(Into::into)
}

pub async fn delete_wiki(state: &AppState, wiki_id: Uuid, user_id: Uuid) -> Result<()> {
    let wiki = permission_service::load_wiki(state, wiki_id).await?;
    // Only the owner can delete a whole wiki.
    if wiki.owner_id != user_id {
        return Err(WikiError::Forbidden);
    }

    // Best-effort removal of the underlying .kbwik files.
    let files: Vec<(Uuid,)> = state
        .db
        .fetch_all_as::<(Uuid,)>("SELECT file_id FROM wiki.pages WHERE wiki_id = $1", params![wiki_id])
        .await?;
    for (f,) in files {
        content_files::delete_page_file(state, wiki.storage_owner_id, f).await;
    }

    // Tombstone + hard delete in one transaction. The cascade drops the wiki's
    // pages/links/categories/members; pages carry no tombstone (the client drops
    // them with the wiki), so no child tombstones are written.
    let mut tx = state.db.begin().await?;
    let seq = sync::next_wiki_seq(&mut tx).await?;
    sync::record_wiki_tombstone(&mut tx, wiki_id, wiki.owner_id, seq).await?;
    tx.execute("DELETE FROM wiki.wikis WHERE id = $1", params![wiki_id]).await?;
    tx.commit().await?;
    Ok(())
}

// ── Membership ──────────────────────────────────────────────────────────────

pub async fn list_members(state: &AppState, wiki_id: Uuid, user_id: Uuid) -> Result<Vec<WikiMemberView>> {
    let wiki = permission_service::require_read(state, wiki_id, user_id).await?;
    let mut out = Vec::new();

    // Owner first.
    if let Some((dn, email)) = user_profile(state, wiki.owner_id).await? {
        out.push(WikiMemberView {
            user_id: wiki.owner_id,
            role: "owner".into(),
            display_name: dn,
            email,
            added_at: wiki.created_at,
        });
    }

    // NOTE: cross-schema join to core.users (PostgreSQL/MySQL only).
    let rows = state
        .db
        .fetch_all_as::<(Uuid, String, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>)>(
            "SELECT m.user_id, m.role, m.added_at, u.display_name, u.email::text \
             FROM wiki.wiki_members m LEFT JOIN core.users u ON u.id = m.user_id \
             WHERE m.wiki_id = $1 ORDER BY m.added_at",
            params![wiki_id],
        )
        .await?;

    for (uid, role, added_at, dn, email) in rows {
        out.push(WikiMemberView {
            user_id: uid,
            role,
            display_name: dn.unwrap_or_default(),
            email: email.unwrap_or_default(),
            added_at,
        });
    }
    Ok(out)
}

pub async fn add_member(
    state: &AppState,
    wiki_id: Uuid,
    user_id: Uuid,
    email: &str,
    role: &str,
) -> Result<()> {
    let wiki = permission_service::require_admin(state, wiki_id, user_id).await?;
    if !wiki.is_shared {
        return Err(WikiError::Validation("cannot add members to a personal wiki".into()));
    }
    let role = match Role::parse(role) {
        Role::Admin | Role::Editor | Role::Reader => role,
        _ => return Err(WikiError::Validation("invalid role".into())),
    };

    // NOTE: core.users lookup (PostgreSQL/MySQL only).
    let target: Option<Uuid> = state
        .db
        .fetch_optional_scalar::<Uuid>(
            "SELECT id FROM core.users WHERE email = $1",
            params![email.trim()],
        )
        .await?;
    let target = target.ok_or_else(|| WikiError::NotFound("user".into()))?;

    if target == wiki.owner_id {
        return Err(WikiError::Conflict("the owner is already a member".into()));
    }

    let upsert = state
        .db
        .backend()
        .upsert("wiki.wiki_members", &["wiki_id", "user_id"], &[Assign::Incoming("role")]);
    let mut tx = state.db.begin().await?;
    tx.execute(
        &format!(
            "INSERT INTO wiki.wiki_members (wiki_id, user_id, role) VALUES ($1, $2, $3){upsert}"
        ),
        params![wiki_id, target, role],
    )
    .await?;
    // Members ride inline in the wiki delta: bump the wiki so the change shows.
    sync::touch_wiki(&mut tx, wiki_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_member(
    state: &AppState,
    wiki_id: Uuid,
    user_id: Uuid,
    member_id: Uuid,
    role: &str,
) -> Result<()> {
    permission_service::require_admin(state, wiki_id, user_id).await?;
    match Role::parse(role) {
        Role::Admin | Role::Editor | Role::Reader => {}
        _ => return Err(WikiError::Validation("invalid role".into())),
    }
    let mut tx = state.db.begin().await?;
    tx.execute(
        "UPDATE wiki.wiki_members SET role = $1 WHERE wiki_id = $2 AND user_id = $3",
        params![role, wiki_id, member_id],
    )
    .await?;
    sync::touch_wiki(&mut tx, wiki_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn remove_member(state: &AppState, wiki_id: Uuid, user_id: Uuid, member_id: Uuid) -> Result<()> {
    permission_service::require_admin(state, wiki_id, user_id).await?;
    let mut tx = state.db.begin().await?;
    tx.execute(
        "DELETE FROM wiki.wiki_members WHERE wiki_id = $1 AND user_id = $2",
        params![wiki_id, member_id],
    )
    .await?;
    sync::touch_wiki(&mut tx, wiki_id).await?;
    tx.commit().await?;
    Ok(())
}

/// (display_name, email) for a user, if present.
///
/// NOTE: reads core.users (PostgreSQL/MySQL only).
pub async fn user_profile(state: &AppState, user_id: Uuid) -> Result<Option<(String, String)>> {
    let row = state
        .db
        .fetch_optional_as::<(Option<String>, Option<String>)>(
            "SELECT display_name, email::text FROM core.users WHERE id = $1",
            params![user_id],
        )
        .await?;
    Ok(row.map(|(dn, email)| (dn.unwrap_or_default(), email.unwrap_or_default())))
}
