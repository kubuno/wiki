//! Full-text page search made identical on the three engines by
//! `kubuno_db::search`: page titles and previews are reduced to Snowball French
//! stems and deaccented IN RUST at write time (stored in `title_norm` /
//! `body_norm`), and a query is put through the same reduction and matched with
//! a portable `LIKE`. This replaces the old PostgreSQL-only
//! `tsvector`/`ts_rank`/`plainto_tsquery`/`unaccent` pipeline.
//!
//! Ranking mirrors the former `setweight A/B`: a hit in the title (weight A)
//! outranks a hit in the preview (weight B). The SQL orders by that score;
//! `rank` is recomputed in Rust for the (already ordered) rows it returns, so
//! the response keeps its shape.
//!
//! Reservation: `pg_trgm`'s typo tolerance is gone — a `LIKE '%stem%'` needs the
//! stem to appear as a substring. Stemming still folds inflections and the
//! normalizer folds accents, so inflected and accented queries still match.

use kubuno_db::params;
use kubuno_db::search::{Field, Query, Weight};
use serde::Serialize;
use uuid::Uuid;

/// The two weighted normalized columns of a page (title outranks preview).
fn fields() -> [Field; 2] {
    [
        Field::new("title_norm", Weight::A),
        Field::new("body_norm", Weight::B),
    ]
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub id:        Uuid,
    pub namespace: String,
    pub title:     String,
    pub slug:      String,
    pub preview:   String,
    /// Relevance score: sum of the weight class of each query term found, higher
    /// first — the portable stand-in for `ts_rank`.
    pub rank:      i64,
}

/// Internal row: the visible columns plus the normalized ones, so the score can
/// be recomputed in Rust for the returned rows.
#[derive(Debug, sqlx::FromRow)]
struct HitRow {
    id:         Uuid,
    namespace:  String,
    title:      String,
    slug:       String,
    preview:    String,
    title_norm: String,
    body_norm:  String,
}

pub async fn search(
    state: &crate::state::AppState,
    wiki_id: Uuid,
    query: &str,
    limit: i64,
) -> crate::errors::Result<Vec<SearchHit>> {
    let limit = limit.clamp(1, 100);
    // Pre-condition placeholders: $1 = wiki_id (`NOT is_deleted` needs no bind).
    // The search filter and score placeholders follow from $2.
    let Some(s) = Query::build(query, &fields(), 2) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT id, namespace, title, slug, preview, title_norm, body_norm \
         FROM wiki.pages \
         WHERE wiki_id = $1 AND NOT is_deleted AND {where_sql} \
         ORDER BY {order_sql} DESC, title ASC \
         LIMIT ${limit_ph}",
        where_sql = s.where_sql,
        order_sql = s.order_sql,
        limit_ph = s.next,
    );
    let mut binds = params![wiki_id];
    binds.extend(s.binds);
    binds.push(limit.into());

    let rows: Vec<HitRow> = state.db.fetch_all_as::<HitRow>(&sql, binds).await?;
    Ok(rows
        .into_iter()
        .map(|r| SearchHit {
            rank: score(&s.terms, &r.title_norm, &r.body_norm),
            id: r.id,
            namespace: r.namespace,
            title: r.title,
            slug: r.slug,
            preview: r.preview,
        })
        .collect())
}

/// Recomputes the row's rank the same way the `ORDER BY` score did: each term
/// found in the title adds weight A, in the preview weight B.
fn score(terms: &[String], title_norm: &str, body_norm: &str) -> i64 {
    let mut total = 0i64;
    for term in terms {
        if title_norm.contains(term.as_str()) {
            total += Weight::A.score() as i64;
        }
        if body_norm.contains(term.as_str()) {
            total += Weight::B.score() as i64;
        }
    }
    total
}
