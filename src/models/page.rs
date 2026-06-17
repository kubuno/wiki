use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Index row for a wiki page. The page source, rendered HTML and revision
/// history live in the `.kbwik` file referenced by `file_id`.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Page {
    pub id:                Uuid,
    pub wiki_id:           Uuid,
    pub namespace:         String,
    pub title:             String,
    pub slug:              String,
    pub file_id:           Uuid,
    pub redirect_to:       Option<String>,
    pub preview:           String,
    pub byte_size:         i32,
    pub current_author_id: Option<Uuid>,
    pub current_rev_at:    DateTime<Utc>,
    pub is_deleted:        bool,
    pub created_at:        DateTime<Utc>,
    pub updated_at:        DateTime<Utc>,
}

/// Lightweight summary used in listings (no file read).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PageSummary {
    pub id:             Uuid,
    pub namespace:      String,
    pub title:          String,
    pub slug:           String,
    pub redirect_to:    Option<String>,
    pub preview:        String,
    pub byte_size:      i32,
    pub current_rev_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SavePageRequest {
    pub namespace: Option<String>,
    pub title:     String,
    pub content:   String,
    #[serde(default)]
    pub comment:   String,
    #[serde(default)]
    pub minor:     bool,
}

#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    pub namespace: Option<String>,
    pub title:     String,
    pub content:   String,
}

// ── Namespaces (MediaWiki-inspired) ──────────────────────────────────────────

/// Canonical namespaces recognised by the wiki engine. `Main` is the implicit
/// (empty-prefix) namespace.
pub const NAMESPACES: &[&str] = &[
    "Main", "Talk", "User", "User_talk", "Wiki", "Wiki_talk",
    "Template", "Category", "File", "Help", "Help_talk",
];

/// Returns the canonical namespace name for an arbitrary user-supplied prefix
/// (case-insensitive, French aliases accepted), or `None` if unknown.
pub fn canonical_namespace(raw: &str) -> Option<&'static str> {
    let n = raw.trim().replace(' ', "_").to_lowercase();
    let canon = match n.as_str() {
        "" | "main" | "principal" | "article" => "Main",
        "talk" | "discussion"                 => "Talk",
        "user" | "utilisateur"                => "User",
        "user_talk" | "discussion_utilisateur" => "User_talk",
        "wiki" | "projet" | "project"         => "Wiki",
        "wiki_talk"                           => "Wiki_talk",
        "template" | "modèle" | "modele"      => "Template",
        "category" | "catégorie" | "categorie" => "Category",
        "file" | "fichier" | "image"          => "File",
        "help" | "aide"                       => "Help",
        "help_talk" | "discussion_aide"       => "Help_talk",
        _ => return None,
    };
    Some(canon)
}

/// The talk namespace paired with a content namespace, if any.
pub fn talk_namespace(ns: &str) -> Option<&'static str> {
    match ns {
        "Main" => Some("Talk"),
        "User" => Some("User_talk"),
        "Wiki" => Some("Wiki_talk"),
        "Help" => Some("Help_talk"),
        _ => None,
    }
}

/// Splits a raw page reference like `Template:Box` into `(namespace, title)`.
/// Defaults to the `Main` namespace when no recognised prefix is present.
pub fn split_namespace(reference: &str) -> (String, String) {
    let reference = reference.trim();
    if let Some((prefix, rest)) = reference.split_once(':') {
        if let Some(canon) = canonical_namespace(prefix) {
            return (canon.to_string(), normalize_title(rest));
        }
    }
    ("Main".to_string(), normalize_title(reference))
}

/// Normalises a display title: trim, collapse whitespace, underscores→spaces,
/// uppercase the first character (MediaWiki convention).
pub fn normalize_title(raw: &str) -> String {
    let collapsed: String = raw
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = collapsed.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Derives the lookup slug from a (already normalised) title: lowercase with
/// spaces turned into underscores. Used as the unique key within a namespace.
pub fn slugify(title: &str) -> String {
    normalize_title(title)
        .to_lowercase()
        .replace(' ', "_")
}

/// Full prefixed title for display, e.g. `Template:Box` or `Main page`.
pub fn prefixed_title(namespace: &str, title: &str) -> String {
    if namespace == "Main" {
        title.to_string()
    } else {
        format!("{}:{}", namespace.replace('_', " "), title)
    }
}
