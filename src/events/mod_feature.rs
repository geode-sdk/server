use crate::types::models::developer::FetchedDeveloper;
use crate::webhook::discord::{DiscordMessage, DiscordWebhook};

pub struct ModFeaturedEvent {
    pub id: String,
    pub name: String,
    pub owner: FetchedDeveloper,
    pub admin: FetchedDeveloper,
    pub base_url: String,
    pub featured: bool,
}

impl DiscordWebhook for ModFeaturedEvent {
    fn to_discord_webhook(&self) -> DiscordMessage {
        let title = match self.featured {
            true => "⭐ Mod featured!",
            false => "😢 Mod unfeatured...",
        };

        DiscordMessage::new().embed(
            title,
            Some(&format!("https://geode-sdk.org/mods/{}\n\nOwned by: [{}](https://github.com/{})\nAction done by: [{}](https://github.com/{})",
                          self.id, self.owner.display_name, self.owner.username, self.admin.display_name, self.admin.username)),
            Some(&format!("{}/v1/mods/{}/logo", self.base_url, self.id)),
        )
    }
}
