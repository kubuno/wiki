//! Instance-wide settings of the wiki module, as the administrator left them in
//! the console.
//!
//! Declared by `module.toml`'s `[[settings]]`, stored in `core.settings`, and read
//! back here through `/internal/modules/wiki/settings` — a module owns its own
//! schema and cannot read the core's tables, and a background worker has no user
//! token for the public config route. The module is named in the URL so the read
//! works whether the instance shares one master secret or a derived one per
//! module.
//!
//! Every field here is read by code that acts on it: a knob that changes nothing
//! is worse than an absent one. The two limits mirror `WikiSettings` in
//! `settings.rs` and, when present, take precedence over the frozen
//! `config.toml` values; the creation policies and the retention window have no
//! file-level counterpart and exist only here.

use serde_json::Value;

/// Who may create a space. Kept as a `Copy` enum rather than a `String` so the
/// whole config stays copyable behind its lock, and so an unknown value coming
/// from the console can never be mistaken for a permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationPolicy {
    /// Any authenticated user.
    Everyone,
    /// Platform administrators only.
    Admins,
}

impl CreationPolicy {
    fn parse(raw: Option<&str>, fallback: Self) -> Self {
        match raw {
            Some("everyone") => Self::Everyone,
            Some("admins")   => Self::Admins,
            _                => fallback,
        }
    }

    /// Whether a user with this administrator flag passes the policy.
    pub fn allows(self, is_admin: bool) -> bool {
        match self {
            Self::Everyone => true,
            Self::Admins   => is_admin,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InstanceConfig {
    /// Ceiling, in bytes, on a page's saved source. A larger save is rejected.
    pub max_content_size: u64,
    /// Deepest template transclusion the renderer follows before it stops
    /// expanding (a guard against recursive/explosive templates).
    pub max_template_depth: u32,
    /// Who may create a space at all.
    pub wiki_creation: CreationPolicy,
    /// Who may create a SHARED space (one others can be invited into). Narrower
    /// than `wiki_creation` in practice: a personal notebook costs the
    /// organisation nothing, a shared space becomes everyone's business.
    pub shared_wiki_creation: CreationPolicy,
    /// Revisions kept per page. Older ones are dropped when a new one is
    /// appended. `0` = keep the whole history.
    pub max_revisions_per_page: u32,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        // Same defaults as `Settings::load` sets for `wiki.*` in settings.rs.
        Self {
            max_content_size:       2_097_152,
            max_template_depth:     16,
            wiki_creation:          CreationPolicy::Everyone,
            shared_wiki_creation:   CreationPolicy::Everyone,
            max_revisions_per_page: 0,
        }
    }
}

impl InstanceConfig {
    /// Maps the core's `{key: value}` object onto the struct. Every read falls
    /// back to the compiled default rather than to a permissive value; an
    /// out-of-range number is treated as a mistake and ignored the same way.
    pub fn from_settings(settings: &Value) -> Self {
        let d = Self::default();
        let int_in = |key: &str, min: i64, max: i64, fallback: i64| -> i64 {
            settings
                .get(key)
                .and_then(Value::as_i64)
                .filter(|n| (min..=max).contains(n))
                .unwrap_or(fallback)
        };
        let policy = |key: &str, fallback: CreationPolicy| {
            CreationPolicy::parse(settings.get(key).and_then(Value::as_str), fallback)
        };
        Self {
            max_content_size:       int_in("max_content_size", 1_024, 1_073_741_824, d.max_content_size as i64) as u64,
            max_template_depth:     int_in("max_template_depth", 1, 100, d.max_template_depth as i64) as u32,
            wiki_creation:          policy("wiki_creation", d.wiki_creation),
            shared_wiki_creation:   policy("shared_wiki_creation", d.shared_wiki_creation),
            max_revisions_per_page: int_in("max_revisions_per_page", 0, 10_000, d.max_revisions_per_page as i64) as u32,
        }
    }
}

/// Reads the instance settings from the core. Any failure yields `None`, so the
/// caller keeps the values it already had rather than reverting to defaults
/// because the core was briefly unreachable.
pub async fn fetch(http: &reqwest::Client, core_url: &str, secret: &str) -> Option<InstanceConfig> {
    let url = format!("{core_url}/internal/modules/wiki/settings");
    let resp = http
        .get(&url)
        .header("X-Internal-Secret", secret)
        .send()
        .await
        .map_err(|e| tracing::warn!(error = %e, "Lecture des réglages d'instance wiki"))
        .ok()?;

    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "Réglages d'instance wiki refusés par le core");
        return None;
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| tracing::warn!(error = %e, "Réglages d'instance wiki : réponse illisible"))
        .ok()?;

    Some(InstanceConfig::from_settings(body.get("settings")?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_keys_keep_the_compiled_defaults() {
        let c = InstanceConfig::from_settings(&json!({}));
        assert_eq!(c.max_content_size, 2_097_152);
        assert_eq!(c.max_template_depth, 16);
    }

    #[test]
    fn admin_values_win_over_defaults() {
        let c = InstanceConfig::from_settings(&json!({
            "max_content_size": 5_242_880, "max_template_depth": 8,
        }));
        assert_eq!(c.max_content_size, 5_242_880);
        assert_eq!(c.max_template_depth, 8);
    }

    #[test]
    fn out_of_range_depth_falls_back() {
        let c = InstanceConfig::from_settings(&json!({ "max_template_depth": 0 }));
        assert_eq!(c.max_template_depth, 16);
    }

    #[test]
    fn creation_policies_default_to_everyone() {
        let c = InstanceConfig::from_settings(&json!({}));
        assert_eq!(c.wiki_creation, CreationPolicy::Everyone);
        assert_eq!(c.shared_wiki_creation, CreationPolicy::Everyone);
        assert!(c.wiki_creation.allows(false));
    }

    #[test]
    fn admins_only_refuses_a_plain_user() {
        let c = InstanceConfig::from_settings(&json!({ "shared_wiki_creation": "admins" }));
        assert!(!c.shared_wiki_creation.allows(false));
        assert!(c.shared_wiki_creation.allows(true));
        // The narrower knob must not have moved the wider one.
        assert!(c.wiki_creation.allows(false));
    }

    #[test]
    fn an_unknown_policy_never_widens_access() {
        let c = InstanceConfig::from_settings(&json!({ "wiki_creation": "anyone-at-all" }));
        assert_eq!(c.wiki_creation, CreationPolicy::Everyone); // the compiled default, not a guess
    }

    #[test]
    fn revision_retention_is_unlimited_by_default() {
        let c = InstanceConfig::from_settings(&json!({}));
        assert_eq!(c.max_revisions_per_page, 0);
        assert_eq!(
            InstanceConfig::from_settings(&json!({ "max_revisions_per_page": 25 })).max_revisions_per_page,
            25
        );
    }
}
