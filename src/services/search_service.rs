//! Full-text page search (French + unaccent), with a trigram/ILIKE fallback so
//! short or partial queries still match titles.

use serde::Serialize;
use uuid::Uuid;

use crate::errors::Result;
use crate::state::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SearchHit {
    pub id:        Uuid,
    pub namespace: String,
    pub title:     String,
    pub slug:      String,
    pub preview:   String,
    pub rank:      f32,
}

pub async fn search(state: &AppState, wiki_id: Uuid, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let like = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
    let rows = sqlx::query_as::<_, SearchHit>(
        "SELECT id, namespace, title, slug, preview, \
                ts_rank(search_vector, plainto_tsquery('french', unaccent($2))) AS rank \
         FROM pages \
         WHERE wiki_id = $1 AND NOT is_deleted \
           AND (search_vector @@ plainto_tsquery('french', unaccent($2)) \
                OR unaccent(title) ILIKE unaccent($3)) \
         ORDER BY rank DESC, title \
         LIMIT $4",
    )
    .bind(wiki_id)
    .bind(q)
    .bind(&like)
    .bind(limit.clamp(1, 100))
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}
