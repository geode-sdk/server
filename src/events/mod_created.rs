use crate::types::models::developer::FetchedDeveloper;
use crate::webhook::discord::{DiscordMessage, DiscordWebhook};

pub struct NewModAcceptedEvent {
    pub id: String,
    pub name: String,
    pub version: String,
    pub owner: FetchedDeveloper,
    pub verified_by: FetchedDeveloper,
    pub base_url: String,
}

pub enum NewModVersionVerification {
    VerifiedDev,
    Admin(FetchedDeveloper),
}

pub struct NewModVersionAcceptedEvent {
    pub id: String,
    pub name: String,
    pub version: String,
    pub owner: FetchedDeveloper,
    pub verified: NewModVersionVerification,
    pub base_url: String,
}

impl DiscordWebhook for NewModAcceptedEvent {
    fn to_discord_webhook(&self) -> DiscordMessage {
        DiscordMessage::new().embed(
            &format!("🎉 New mod: {} {}", self.name, self.version),
            Some(&format!("https://geode-sdk.org/mods/{}\n\nOwned by [{}](https://github.com/{})\nAccepted by [{}](https://github.com/{})",
                          self.id, self.owner.display_name, self.owner.username, self.verified_by.display_name, self.verified_by.username)),
            Some(&format!("{}/v1/mods/{}/logo", self.base_url, self.id)),
        )
    }
}

impl DiscordWebhook for NewModVersionAcceptedEvent {
    fn to_discord_webhook(&self) -> DiscordMessage {
        let accepted_msg = match &self.verified {
            NewModVersionVerification::VerifiedDev => String::from("Developer is verified"),
            NewModVersionVerification::Admin(admin) => format!(
                "Accepted by [{}](https://github.com/{})",
                admin.display_name, admin.username
            ),
        };

        DiscordMessage::new().embed(
            &format!("🎉 Updated {} to {}", self.name, self.version),
            Some(&format!(
                "https://geode-sdk.org/mods/{}\n\nOwned by [{}](https://github.com/{})\n{}",
                self.id, self.owner.display_name, self.owner.username, accepted_msg
            )),
            Some(&format!("{}/v1/mods/{}/logo", self.base_url, self.id)),
        )
    }
}
