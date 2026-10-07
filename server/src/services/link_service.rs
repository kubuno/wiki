//! Internal link graph queries: backlinks ("what links here").

use kubuno_db::params;
use serde::Serialize;
use uuid::Uuid;

use crate::errors::Result;
use crate::state::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Backlink {
    pub id:        Uuid,
    pub namespace: String,
    pub title:     String,
    pub slug:      String,
}

/// Pages that link to the given page (resolved links only).
pub async fn backlinks(state: &AppState, wiki_id: Uuid, page_id: Uuid) -> Result<Vec<Backlink>> {
    let rows = state
        .db
        .fetch_all_as::<Backlink>(
            "SELECT p.id, p.namespace, p.title, p.slug \
             FROM wiki.page_links l JOIN wiki.pages p ON p.id = l.source_page_id \
             WHERE l.wiki_id = $1 AND l.target_page_id = $2 AND NOT p.is_deleted \
             ORDER BY p.namespace, p.title",
            params![wiki_id, page_id],
        )
        .await?;
    Ok(rows)
}
