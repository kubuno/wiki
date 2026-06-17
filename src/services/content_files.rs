//! Storage of wiki PAGE content in the `drive` module (not in the database).
//!
//! Kubuno wiki format — MIME `application/vnd.kubuno.wiki+json`, extension
//! `.kbwik`, gzipped JSON. The database only keeps an index row (`pages`): the
//! `file_id` reference, a truncated `preview` and the derived FTS `search_vector`.
//!
//! One `.kbwik` file = one page, and it is fully self-contained: it carries the
//! current source, the rendered HTML cache and the **whole revision history**.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::io::{Read as _, Write as _};
use uuid::Uuid;

use crate::{errors::WikiError, state::AppState};

pub const WIKI_MIME: &str = "application/vnd.kubuno.wiki+json";
pub const WIKI_EXT:  &str = "kbwik";

/// A single stored revision of a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    pub rev_id:      Uuid,
    pub author_id:   Option<Uuid>,
    pub author_name: String,
    pub ts:          String, // RFC3339
    #[serde(default)]
    pub comment:     String,
    #[serde(default)]
    pub minor:       bool,
    pub content:     String,
    pub size:        i64,
}

/// On-disk envelope for a `.kbwik` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageEnvelope {
    #[serde(default = "default_version")]
    pub version:      u32,
    pub namespace:    String,
    pub title:        String,
    pub content:      String,
    #[serde(default)]
    pub content_html: String,
    #[serde(default)]
    pub redirect:     Option<String>,
    #[serde(default)]
    pub revisions:    Vec<Revision>,
}

fn default_version() -> u32 { 1 }

impl PageEnvelope {
    pub fn current_content(&self) -> &str {
        &self.content
    }
}

// ── Compression (gzip) ──────────────────────────────────────────────────────

fn gzip(raw: &[u8]) -> Result<Vec<u8>, WikiError> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(raw).map_err(|e| WikiError::Internal(anyhow::anyhow!(e)))?;
    enc.finish().map_err(|e| WikiError::Internal(anyhow::anyhow!(e)))
}

fn gunzip(raw: &[u8]) -> Result<Vec<u8>, WikiError> {
    if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
        let mut dec = flate2::read::GzDecoder::new(raw);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).map_err(|e| WikiError::Internal(anyhow::anyhow!(e)))?;
        Ok(out)
    } else {
        Ok(raw.to_vec())
    }
}

// ── File naming & folder layout ─────────────────────────────────────────────

fn kb_file_name(title: &str) -> String {
    let base = title.trim();
    let base = std::path::Path::new(base).file_stem().and_then(|s| s.to_str()).unwrap_or(base);
    let base = if base.is_empty() { "Page" } else { base };
    format!("{base}.{WIKI_EXT}")
}

/// Drive folder path for a page: `Wiki/<wiki_slug>` for the Main namespace,
/// `Wiki/<wiki_slug>/<Namespace>` otherwise.
fn folder_path(wiki_slug: &str, namespace: &str) -> String {
    if namespace == "Main" {
        format!("Wiki/{wiki_slug}")
    } else {
        format!("Wiki/{wiki_slug}/{namespace}")
    }
}

// ── CRUD on `.kbwik` files ──────────────────────────────────────────────────

pub async fn create_page_file(
    state: &AppState,
    storage_owner_id: Uuid,
    wiki_slug: &str,
    envelope: &PageEnvelope,
) -> Result<Uuid, WikiError> {
    let folder = state
        .files_client
        .ensure_folder_path(storage_owner_id, &folder_path(wiki_slug, &envelope.namespace), true, Some("BookMarked"))
        .await
        .map_err(WikiError::Internal)?;

    let raw = serde_json::to_vec(envelope).map_err(|e| WikiError::Internal(anyhow::anyhow!(e)))?;
    let gz = gzip(&raw)?;
    let file = state
        .files_client
        .create_file_with_content(
            storage_owner_id,
            Some(folder.id),
            &kb_file_name(&envelope.title),
            WIKI_MIME,
            Bytes::from(gz),
            Some(serde_json::json!({ "module": "wiki", "subtype": "page", "namespace": envelope.namespace })),
            false,
        )
        .await
        .map_err(WikiError::Internal)?;
    Ok(file.id)
}

pub async fn read_page_file(
    state: &AppState,
    storage_owner_id: Uuid,
    file_id: Uuid,
) -> Result<PageEnvelope, WikiError> {
    let (_info, raw) = state
        .files_client
        .get_file_content(storage_owner_id, file_id)
        .await
        .map_err(WikiError::Internal)?;
    let json = gunzip(&raw)?;
    serde_json::from_slice::<PageEnvelope>(&json)
        .map_err(|e| WikiError::Internal(anyhow::anyhow!("unreadable .kbwik: {e}")))
}

pub async fn write_page_file(
    state: &AppState,
    storage_owner_id: Uuid,
    file_id: Uuid,
    envelope: &PageEnvelope,
) -> Result<(), WikiError> {
    let raw = serde_json::to_vec(envelope).map_err(|e| WikiError::Internal(anyhow::anyhow!(e)))?;
    let gz = gzip(&raw)?;
    state
        .files_client
        .update_file_content(storage_owner_id, file_id, Bytes::from(gz))
        .await
        .map_err(WikiError::Internal)
        .map(|_| ())
}

pub async fn delete_page_file(state: &AppState, storage_owner_id: Uuid, file_id: Uuid) {
    let _ = state.files_client.delete_file(storage_owner_id, file_id).await;
}

/// Best-effort rename of the underlying `.kbwik` file so it carries `<title>`.
pub async fn rename_page_file(state: &AppState, storage_owner_id: Uuid, file_id: Uuid, title: &str) {
    crate::files_client::set_title(&state.files_client, storage_owner_id, file_id, title, WIKI_EXT).await
}

/// Truncated plain-text preview (not the full content) for listings & FTS.
pub fn make_preview(content: &str) -> String {
    let flat: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(400).collect()
}
