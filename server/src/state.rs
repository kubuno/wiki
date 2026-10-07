use crate::config::instance::InstanceConfig;
use crate::config::Settings;
use crate::files_client::FilesClient;
use reqwest::Client;
use kubuno_db::DbPool;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub db:           DbPool,
    pub settings:     Arc<Settings>,
    pub http:         Client,
    pub files_client: Arc<FilesClient>,
    /// Instance settings from the admin console, refreshed in the background so
    /// an edit takes effect without restarting the module. Read through
    /// [`AppState::instance`], never locked directly by callers.
    pub instance:     Arc<RwLock<InstanceConfig>>,
}

impl AppState {
    /// A snapshot of the current instance settings. Falls back to the compiled
    /// defaults if the lock was poisoned by a panicking writer — a lost value
    /// must never take a protection down.
    pub fn instance(&self) -> InstanceConfig {
        self.instance.read().map(|c| *c).unwrap_or_default()
    }

    /// Publish an event to the core bus (best-effort, errors are logged only).
    pub async fn publish(&self, event: serde_json::Value) {
        if let Err(e) = crate::events::publish_event(
            &self.http,
            &self.settings.core.url,
            &self.settings.core.internal_secret,
            event,
        ).await {
            tracing::warn!(error = %e, "Failed to publish event");
        }
    }
}
