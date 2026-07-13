use crate::webhook::discord::{DiscordMessage, DiscordWebhook};

pub struct NewThreadComment {
    pub mod_id: String,
    pub mod_version: String,
    pub username: String,
}

impl DiscordWebhook for NewThreadComment {
    fn to_discord_webhook(&self) -> DiscordMessage {
        DiscordMessage::new().embed(
            &format!(
                "✅ New comment on thread {} v{}",
                self.mod_id, self.mod_version,
            ),
            Some(&format!(
                "https://geode-sdk.org/mods/{}?version={}\n\nComment posted by {}",
                self.mod_id, self.mod_version, self.username
            )),
            None,
        )
    }
}
