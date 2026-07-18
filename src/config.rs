use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use moka::future::Cache;
use tokio::sync::mpsc::Sender;

use crate::{
    endpoints::mods::IndexQueryParams,
    s3_worker::S3WorkerTask,
    storage::{LocalBackend, PrivateDisk, PublicDisk, S3Backend, S3Configuration},
    types::{
        api::{ApiResponse, PaginatedData},
        models::mod_entity::Mod,
    },
};

#[derive(Clone)]
pub struct AppData {
    db: sqlx::postgres::PgPool,
    app_url: String,
    front_url: String,
    github: GitHubClientData,
    webhook_url: String,
    index_admin_webhook_url: String,
    static_storage: PublicDisk,
    public_storage: PublicDisk,
    private_storage: PrivateDisk,
    mod_storage: Option<PublicDisk>,
    disable_downloads: bool,
    max_download_mb: u32,
    port: u16,
    debug: bool,

    mods_cache: Cache<IndexQueryParams, ApiResponse<PaginatedData<Mod>>>,
    http_client: reqwest::Client,

    s3_sender: OnceLock<Sender<S3WorkerTask>>,
}

#[derive(Clone)]
pub struct GitHubClientData {
    client_id: String,
    client_secret: String,
}

pub async fn build_config() -> anyhow::Result<AppData> {
    let env_url = dotenvy::var("DATABASE_URL")?;

    let pg_connections =
        dotenvy::var("DATABASE_CONNECTIONS").map_or(10, |x: String| x.parse::<u32>().unwrap_or(10));

    let pool = sqlx::postgres::PgPoolOptions::default()
        .max_connections(pg_connections)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&env_url)
        .await?;
    let port = dotenvy::var("PORT").map_or(8080, |x: String| x.parse::<u16>().unwrap());
    let debug = dotenvy::var("APP_DEBUG").unwrap_or("0".to_string()) == "1";
    let app_url = dotenvy::var("APP_URL").unwrap_or("http://localhost".to_string());
    let front_url = dotenvy::var("FRONT_URL").unwrap_or("http://localhost".to_string());
    let github_client = dotenvy::var("GITHUB_CLIENT_ID").unwrap_or("".to_string());
    let github_secret = dotenvy::var("GITHUB_CLIENT_SECRET").unwrap_or("".to_string());
    let webhook_url = dotenvy::var("DISCORD_WEBHOOK_URL").unwrap_or("".to_string());
    let index_admin_webhook_url =
        dotenvy::var("INDEX_ADMIN_DISCORD_WEBHOOK_URL").unwrap_or("".to_string());
    let disable_downloads =
        dotenvy::var("DISABLE_DOWNLOAD_COUNTS").unwrap_or("0".to_string()) == "1";
    let max_download_mb = dotenvy::var("MAX_MOD_FILESIZE_MB")
        .unwrap_or("250".to_string())
        .parse::<u32>()
        .unwrap_or(250);
    let mods_cache = Cache::builder()
        .max_capacity(256)
        .time_to_idle(Duration::from_mins(5))
        .time_to_live(Duration::from_mins(10))
        .build();

    let mod_storage = if let Some(s3_config) = S3Configuration::from_env()? {
        let backend = Arc::new(S3Backend::new(&s3_config)?);
        Some(PublicDisk::new(backend, s3_config.public_url))
    } else {
        None
    };

    Ok(AppData {
        db: pool,
        app_url: app_url.clone(),
        front_url,
        github: GitHubClientData {
            client_id: github_client,
            client_secret: github_secret,
        },
        webhook_url,
        index_admin_webhook_url,
        static_storage: PublicDisk::new(
            Arc::new(LocalBackend::new("static")),
            format!("{app_url}/static"),
        ),
        public_storage: PublicDisk::new(
            Arc::new(LocalBackend::new("storage/public")),
            format!("{app_url}/storage"),
        ),
        private_storage: PrivateDisk::new(Arc::new(LocalBackend::new("storage/private"))),
        mod_storage,
        disable_downloads,
        max_download_mb,
        port,
        debug,
        mods_cache,
        http_client: reqwest::Client::builder()
            .pool_max_idle_per_host(4)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .build()?,
        s3_sender: OnceLock::new(),
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

    pub fn index_admin_webhook_url(&self) -> &str {
        &self.index_admin_webhook_url
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

    pub fn static_storage(&self) -> &PublicDisk {
        &self.static_storage
    }

    pub fn public_storage(&self) -> &PublicDisk {
        &self.public_storage
    }

    pub fn private_storage(&self) -> &PrivateDisk {
        &self.private_storage
    }

    pub fn mod_storage(&self) -> Option<&PublicDisk> {
        self.mod_storage.as_ref()
    }

    pub fn mods_cache(&self) -> &Cache<IndexQueryParams, ApiResponse<PaginatedData<Mod>>> {
        &self.mods_cache
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    pub fn init_s3_sender(&self, sender: Sender<S3WorkerTask>) {
        self.s3_sender
            .set(sender)
            .expect("init_s3_sender must be called only once");
    }

    pub fn send_s3_task(&self, task: S3WorkerTask) -> bool {
        if let Some(sender) = self.s3_sender.get() {
            sender.try_send(task).is_ok()
        } else {
            false
        }
    }
}
