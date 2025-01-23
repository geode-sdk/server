#[derive(Clone)]
pub struct AppData {
    db: sqlx::postgres::PgPool,
    app_url: String,
    front_url: String,
    github: GitHubClientData,
    webhook_url: String,
    discord: DiscordForumData,
    disable_downloads: bool,
    max_download_mb: u32,
    port: u16,
    debug: bool,
}

#[derive(Clone)]
pub struct GitHubClientData {
    client_id: String,
    client_secret: String,
}

#[derive(Clone)]
pub struct DiscordForumData {
    guild_id: u64,
    channel_id: u64,
    bot_token: String,
}

pub async fn build_config() -> anyhow::Result<AppData> {
    let env_url = dotenvy::var("DATABASE_URL")?;

    let pool = sqlx::postgres::PgPoolOptions::default()
        .max_connections(10)
        .connect(&env_url)
        .await?;
    let port = dotenvy::var("PORT").map_or(8080, |x: String| x.parse::<u16>().unwrap());
    let debug = dotenvy::var("APP_DEBUG").unwrap_or("0".to_string()) == "1";
    let app_url = dotenvy::var("APP_URL").unwrap_or("http://localhost".to_string());
    let front_url = dotenvy::var("FRONT_URL").unwrap_or("http://localhost".to_string());
    let github_client = dotenvy::var("GITHUB_CLIENT_ID").unwrap_or("".to_string());
    let github_secret = dotenvy::var("GITHUB_CLIENT_SECRET").unwrap_or("".to_string());
    let webhook_url = dotenvy::var("DISCORD_WEBHOOK_URL").unwrap_or("".to_string());
    let guild_id = dotenvy::var("DISCORD_GUILD_ID")
        .unwrap_or("0".to_string())
        .parse::<u64>()
        .unwrap_or(0);
    let channel_id = dotenvy::var("DISCORD_CHANNEL_ID")
        .unwrap_or("0".to_string())
        .parse::<u64>()
        .unwrap_or(0);
    let bot_token = dotenvy::var("DISCORD_BOT_TOKEN").unwrap_or("".to_string());
    let disable_downloads =
        dotenvy::var("DISABLE_DOWNLOAD_COUNTS").unwrap_or("0".to_string()) == "1";
    let max_download_mb = dotenvy::var("MAX_MOD_FILESIZE_MB")
        .unwrap_or("250".to_string())
        .parse::<u32>()
        .unwrap_or(250);

    Ok(AppData {
        db: pool,
        app_url,
        front_url,
        github: GitHubClientData {
            client_id: github_client,
            client_secret: github_secret,
        },
        webhook_url,
        discord: DiscordForumData {
            guild_id,
            channel_id,
            bot_token,
        },
        disable_downloads,
        max_download_mb,
        port,
        debug,
    })
}

impl GitHubClientData {
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn client_secret(&self) -> &str {
        &self.client_secret
    }
}

impl DiscordForumData {
    pub fn is_valid(&self) -> bool {
        self.guild_id != 0 && self.channel_id != 0 && !self.bot_token.is_empty()
    }

    pub fn guild_id(&self) -> u64 {
        self.guild_id
    }

    pub fn channel_id(&self) -> u64 {
        self.channel_id
    }

    pub fn bot_auth(&self) -> String {
        format!("Bot {}", self.bot_token)
    }
}

impl AppData {
    pub fn db(&self) -> &sqlx::postgres::PgPool {
        &self.db
    }

    pub fn app_url(&self) -> &str {
        &self.app_url
    }

    pub fn front_url(&self) -> &str {
        &self.front_url
    }

    pub fn github(&self) -> &GitHubClientData {
        &self.github
    }

    pub fn webhook_url(&self) -> &str {
        &self.webhook_url
    }

    pub fn discord(&self) -> &DiscordForumData {
        &self.discord
    }

    pub fn disable_downloads(&self) -> bool {
        self.disable_downloads
    }

    pub fn max_download_mb(&self) -> u32 {
        self.max_download_mb
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn debug(&self) -> bool {
        self.debug
    }
}
