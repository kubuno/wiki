use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;

pub async fn publish_event(
    client: &Client,
    core_url: &str,
    internal_secret: &str,
    event: Value,
) -> Result<()> {
    let url = format!("{core_url}/internal/events/publish");
    client
        .post(&url)
        .header("X-Internal-Secret", internal_secret)
        .json(&event)
        .send()
        .await?;
    Ok(())
}

fn page_event(kind: &str, wiki_id: Uuid, page_id: Uuid, user_id: Uuid) -> Value {
    json!({
        "type": kind,
        "payload": {
            "wiki_id":   wiki_id,
            "page_id":   page_id,
            "user_id":   user_id,
            "module_id": "wiki"
        }
    })
}

pub fn page_created_event(wiki_id: Uuid, page_id: Uuid, user_id: Uuid) -> Value {
    page_event("WikiPageCreated", wiki_id, page_id, user_id)
}

pub fn page_updated_event(wiki_id: Uuid, page_id: Uuid, user_id: Uuid) -> Value {
    page_event("WikiPageUpdated", wiki_id, page_id, user_id)
}

pub fn page_deleted_event(wiki_id: Uuid, page_id: Uuid, user_id: Uuid) -> Value {
    page_event("WikiPageDeleted", wiki_id, page_id, user_id)
}
