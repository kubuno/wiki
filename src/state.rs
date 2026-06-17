use crate::config::Settings;
use crate::files_client::FilesClient;
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db:           PgPool,
    pub settings:     Arc<Settings>,
    pub http:         Client,
    pub files_client: Arc<FilesClient>,
}

impl AppState {
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
