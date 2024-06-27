use serde_json::json;

use crate::types::models::developer::{Developer, FetchedDeveloper};

pub async fn send_webhook(
    id: String,
    name: String,
    version: String,
    update: bool,
    owner: Developer,
    verified_by: FetchedDeveloper,
    webhook_url: String,
    base_url: String,
) {
    // webhook not configured, exit function
    if webhook_url.is_empty() {
        log::error!("Discord Webhook is not configured. Not sending webhook.");
        return;
    }
    tokio::spawn(async move {
        let webhook = json!({
            "embeds": [
                {
                    "title": if !update { format!("Added {} {}", name, version) } else { format!("Updated {} {}", name, version) },
                    "description": format!(
                        "https://geode-sdk.org/mods/{}\n\nOwned by: [{}](https://github.com/{})\nAccepted by: [{}](https://github.com/{})",
                        id, owner.display_name, owner.username, verified_by.display_name, verified_by.username
                    ),
                    "thumbnail": {
                        "url": format!("{}/v1/mods/{}/logo", base_url, id)
                    }
                }
            ]
        });

        let _ = reqwest::Client::new()
            .post(webhook_url)
            .json(&webhook)
            .send()
            .await;
    });
}
