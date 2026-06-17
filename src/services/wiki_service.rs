//! Wiki space lifecycle: create / list / update / delete, plus membership
//! management for shared wikis. User identities are resolved against `core.users`.

use uuid::Uuid;

use crate::errors::{Result, WikiError};
use crate::models::member::{Role, WikiMemberView};
use crate::models::wiki::{CreateWikiRequest, UpdateWikiRequest, Wiki, WikiView};
use crate::services::{content_files, permission_service};
use crate::state::AppState;

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
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM wikis WHERE owner_id = $1 AND slug = $2)",
        )
        .bind(owner_id)
        .bind(&candidate)
        .fetch_one(&state.db)
        .await?;
        if !exists {
            return Ok(candidate);
        }
    }
    unreachable!()
}

pub async fn list_wikis(state: &AppState, user_id: Uuid) -> Result<Vec<WikiView>> {
    let wikis = sqlx::query_as::<_, Wiki>(
        "SELECT DISTINCT w.* FROM wikis w \
         LEFT JOIN wiki_members m ON m.wiki_id = w.id AND m.user_id = $1 \
         WHERE w.owner_id = $1 OR m.user_id = $1 \
         ORDER BY w.name",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::with_capacity(wikis.len());
    for w in wikis {
        let role = permission_service::effective_role(state, &w, user_id).await?;
        let page_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pages WHERE wiki_id = $1 AND NOT is_deleted",
        )
        .bind(w.id)
        .fetch_one(&state.db)
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

    let wiki = sqlx::query_as::<_, Wiki>(
        "INSERT INTO wikis (owner_id, storage_owner_id, slug, name, description, is_shared) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(user_id)
    .bind(storage_owner)
    .bind(&slug)
    .bind(name)
    .bind(req.description.trim())
    .bind(req.is_shared)
    .fetch_one(&state.db)
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
    let page_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages WHERE wiki_id = $1 AND NOT is_deleted",
    )
    .bind(wiki_id)
    .fetch_one(&state.db)
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
    let wiki = sqlx::query_as::<_, Wiki>(
        "UPDATE wikis SET \
            name        = COALESCE($2, name), \
            description = COALESCE($3, description) \
         WHERE id = $1 RETURNING *",
    )
    .bind(wiki_id)
    .bind(req.name.as_deref().map(str::trim))
    .bind(req.description.as_deref().map(str::trim))
    .fetch_one(&state.db)
    .await?;
    Ok(wiki)
}

pub async fn delete_wiki(state: &AppState, wiki_id: Uuid, user_id: Uuid) -> Result<()> {
    let wiki = permission_service::load_wiki(state, wiki_id).await?;
    // Only the owner can delete a whole wiki.
    if wiki.owner_id != user_id {
        return Err(WikiError::Forbidden);
    }

    // Best-effort removal of the underlying .kbwik files.
    let files: Vec<Uuid> = sqlx::query_scalar("SELECT file_id FROM pages WHERE wiki_id = $1")
        .bind(wiki_id)
        .fetch_all(&state.db)
        .await?;
    for f in files {
        content_files::delete_page_file(state, wiki.storage_owner_id, f).await;
    }

    sqlx::query("DELETE FROM wikis WHERE id = $1")
        .bind(wiki_id)
        .execute(&state.db)
        .await?;
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

    let rows = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>)>(
        "SELECT m.user_id, m.role, m.added_at, u.display_name, u.email::text \
         FROM wiki_members m LEFT JOIN core.users u ON u.id = m.user_id \
         WHERE m.wiki_id = $1 ORDER BY m.added_at",
    )
    .bind(wiki_id)
    .fetch_all(&state.db)
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

    let target: Option<Uuid> = sqlx::query_scalar("SELECT id FROM core.users WHERE email = $1")
        .bind(email.trim())
        .fetch_optional(&state.db)
        .await?;
    let target = target.ok_or_else(|| WikiError::NotFound("user".into()))?;

    if target == wiki.owner_id {
        return Err(WikiError::Conflict("the owner is already a member".into()));
    }

    sqlx::query(
        "INSERT INTO wiki_members (wiki_id, user_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (wiki_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(wiki_id)
    .bind(target)
    .bind(role)
    .execute(&state.db)
    .await?;
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
    sqlx::query("UPDATE wiki_members SET role = $3 WHERE wiki_id = $1 AND user_id = $2")
        .bind(wiki_id)
        .bind(member_id)
        .bind(role)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn remove_member(state: &AppState, wiki_id: Uuid, user_id: Uuid, member_id: Uuid) -> Result<()> {
    permission_service::require_admin(state, wiki_id, user_id).await?;
    sqlx::query("DELETE FROM wiki_members WHERE wiki_id = $1 AND user_id = $2")
        .bind(wiki_id)
        .bind(member_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// (display_name, email) for a user, if present.
pub async fn user_profile(state: &AppState, user_id: Uuid) -> Result<Option<(String, String)>> {
    let row = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT display_name, email::text FROM core.users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(dn, email)| (dn.unwrap_or_default(), email.unwrap_or_default())))
}
