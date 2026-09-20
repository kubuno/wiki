//! Delta-sync plumbing shared by the services and the delta handler.
//!
//! The local-first pull (wikis / pages) rests on a monotonic `change_seq` per
//! record and a tombstone per hard-deleted wiki. On PostgreSQL that used to be a
//! `SEQUENCE` plus `BEFORE UPDATE` / `AFTER DELETE` triggers; here it is the
//! portable [`kubuno_db::journal`] primitive, driven from Rust at every write
//! site. This module holds the literal table / domain names those calls take —
//! all `&'static str`, never request data — so the write sites read uniformly
//! and a rename happens in one place.
//!
//! Two entities are versioned: **wikis** and **pages**.
//!
//! * `wiki_members` bump their **wiki** (members ride inline in the wiki delta).
//! * Pages are SOFT-deleted (the `is_deleted` flag): a deletion is just a
//!   `modified` change carrying `is_deleted = true`, so pages never tombstone.
//!   Only a wiki HARD-delete writes a tombstone; the client then drops the wiki
//!   and its pages.

use uuid::Uuid;

/// One shared counter table per schema; `next_seq` keys it by domain.
pub const CHANGE_COUNTER: &str = "wiki.change_counter";

pub const WIKIS_TABLE: &str = "wiki.wikis";
pub const PAGES_TABLE: &str = "wiki.pages";
pub const WIKI_TOMBSTONES: &str = "wiki.wiki_tombstones";

/// Logical counter domains (the row keys in `change_counter`).
pub const WIKI_DOMAIN: &str = "wikis";
pub const PAGE_DOMAIN: &str = "pages";

/// The next monotonic sequence for the **wikis** domain, taken inside `tx`.
pub async fn next_wiki_seq(tx: &mut kubuno_db::DbTx) -> Result<i64, sqlx::Error> {
    kubuno_db::journal::next_seq(tx, CHANGE_COUNTER, WIKI_DOMAIN).await
}

/// The next monotonic sequence for the **pages** domain, taken inside `tx`.
pub async fn next_page_seq(tx: &mut kubuno_db::DbTx) -> Result<i64, sqlx::Error> {
    kubuno_db::journal::next_seq(tx, CHANGE_COUNTER, PAGE_DOMAIN).await
}

/// Bumps a **wiki** to a fresh sequence — the portable replacement for the old
/// child-triggered no-op `UPDATE`. Called after any write to a wiki member.
pub async fn touch_wiki(tx: &mut kubuno_db::DbTx, wiki_id: Uuid) -> Result<(), sqlx::Error> {
    kubuno_db::journal::touch(tx, WIKIS_TABLE, CHANGE_COUNTER, WIKI_DOMAIN, "id", wiki_id)
        .await
        .map(|_| ())
}

/// Writes a **wiki** tombstone in the same transaction as its hard delete.
pub async fn record_wiki_tombstone(
    tx: &mut kubuno_db::DbTx,
    id: Uuid,
    owner_id: Uuid,
    seq: i64,
) -> Result<(), sqlx::Error> {
    kubuno_db::journal::record_tombstone(tx, WIKI_TOMBSTONES, id, owner_id, seq).await
}
