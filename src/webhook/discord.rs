use serde::Serialize;

pub trait DiscordWebhook {
    fn to_discord_webhook(&self) -> DiscordMessage;
}

#[derive(Serialize, Debug, Clone)]
pub struct DiscordMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    embeds: Vec<DiscordMessageEmbed>,
}

impl DiscordMessage {
    pub fn new() -> DiscordMessage {
        DiscordMessage {
            embeds: vec![],
            content: None,
        }
    }

    pub fn content(self, content: &str) -> Self {
        DiscordMessage {
            embeds: self.embeds,
            content: Some(content.into())
        }
    }

    pub fn embed(
        self,
        title: &str,
        description: Option<&str>,
        thumbnail_url: Option<&str>,
    ) -> Self {
        if self.embeds.len() == 10 {
            return DiscordMessage {
                content: self.content,
                embeds: self.embeds
            };
        }

        let mut embed = DiscordMessageEmbed {
            title: String::from(title),
            description: None,
            thumbnail: None,
        };

        if let Some(s) = description {
            embed.description = Some(String::from(s));
        }

        if let Some(s) = thumbnail_url {
            embed.thumbnail = Some(DiscordMessageEmbedThumbnail {
                url: String::from(s),
            });
        }

        let mut embeds = self.embeds;
        embeds.push(embed);

        DiscordMessage {
            content: self.content,
            embeds
        }
    }

    pub fn send(&self, url: &str) {
        if url.is_empty() {
            log::error!("Not sending webhook since URL is empty");
            return;
        }

        log::debug!("Sending {:?} to webhook url {}", self, url);
        let url = String::from(url);
        let copy = self.clone();

        tokio::spawn(async move {

            if let Err(e) = reqwest::Client::new()
                .post(&url)
                .json(&copy)
                .send()
                .await
            {
                log::error!("Failed to broadcast Discord webhook {}: {}", url, e);
            }
        });
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct DiscordMessageEmbed {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail: Option<DiscordMessageEmbedThumbnail>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DiscordMessageEmbedThumbnail {
    url: String,
}
