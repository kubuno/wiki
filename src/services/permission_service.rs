//! Effective-role resolution for wiki access control.
//!
//! - Personal wiki (`is_shared = false`): only the owner has access (as `Owner`).
//! - Shared wiki: the owner is `Owner`; other users get their `wiki_members.role`,
//!   or `None` if they are not members.

use uuid::Uuid;

use crate::errors::{Result, WikiError};
use crate::models::member::Role;
use crate::models::wiki::Wiki;
use crate::state::AppState;

pub async fn load_wiki(state: &AppState, wiki_id: Uuid) -> Result<Wiki> {
    sqlx::query_as::<_, Wiki>("SELECT * FROM wikis WHERE id = $1")
        .bind(wiki_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| WikiError::NotFound("wiki".into()))
}

pub async fn effective_role(state: &AppState, wiki: &Wiki, user_id: Uuid) -> Result<Role> {
    if wiki.owner_id == user_id {
        return Ok(Role::Owner);
    }
    if !wiki.is_shared {
        return Ok(Role::None);
    }
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM wiki_members WHERE wiki_id = $1 AND user_id = $2",
    )
    .bind(wiki.id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(role.map(|r| Role::parse(&r)).unwrap_or(Role::None))
}

/// Loads the wiki and the user's role, erroring with NotFound when the user
/// cannot even read it (avoids leaking the existence of private wikis).
pub async fn authorize(state: &AppState, wiki_id: Uuid, user_id: Uuid) -> Result<(Wiki, Role)> {
    let wiki = load_wiki(state, wiki_id).await?;
    let role = effective_role(state, &wiki, user_id).await?;
    if !role.can_read() {
        return Err(WikiError::NotFound("wiki".into()));
    }
    Ok((wiki, role))
}

pub async fn require_read(state: &AppState, wiki_id: Uuid, user_id: Uuid) -> Result<Wiki> {
    let (wiki, _role) = authorize(state, wiki_id, user_id).await?;
    Ok(wiki)
}

pub async fn require_edit(state: &AppState, wiki_id: Uuid, user_id: Uuid) -> Result<Wiki> {
    let (wiki, role) = authorize(state, wiki_id, user_id).await?;
    if !role.can_edit() {
        return Err(WikiError::Forbidden);
    }
    Ok(wiki)
}

pub async fn require_admin(state: &AppState, wiki_id: Uuid, user_id: Uuid) -> Result<Wiki> {
    let (wiki, role) = authorize(state, wiki_id, user_id).await?;
    if !role.can_admin() {
        return Err(WikiError::Forbidden);
    }
    Ok(wiki)
}
