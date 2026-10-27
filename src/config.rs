use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use moka::future::Cache;
use tokio::sync::mpsc::Sender;
use tracing::field::debug;

use crate::{
    auth::github::GithubClient,
    dns::ValidateDnsResolver,
    http::{self, response::ApiResponse},
    s3_worker::{QueuedS3Task, S3WorkerTask},
    storage::{LocalBackend, PrivateDisk, PublicDisk, S3Backend, S3Configuration},
    types::{
        api::PaginatedData,
        models::mod_entity::{IndexQueryParams, Mod},
    },
};

#[derive(Clone)]
pub struct AppData {
    db: sqlx::postgres::PgPool,
    app_url: String,
    front_url: String,
    github_client: GithubClient,
    webhook_url: String,
    index_admin_webhook_url: String,
    static_storage: PublicDisk,
    public_storage: PublicDisk,
    private_storage: PrivateDisk,
    cdn_storage: Option<PublicDisk>,
    disable_downloads: bool,
    max_download_mb: u32,
    port: u16,
    debug: bool,

    mods_cache: Cache<IndexQueryParams, ApiResponse<PaginatedData<Mod>>>,
    http_client: reqwest::Client,
    check_dns_http_client: reqwest::Client,

    s3_sender: OnceLock<Sender<QueuedS3Task>>,
}

impl AppData {
    pub fn builder(pool: sqlx::postgres::PgPool) -> AppDataBuilder {
        AppDataBuilder::start(pool)
    }
}

pub struct AppDataBuilder {
    db: sqlx::postgres::PgPool,
    app_url: Option<String>,
    front_url: Option<String>,
    port: Option<u16>,
    github_client: Option<GithubClient>,
    webhook_url: Option<String>,
    index_admin_webhook_url: Option<String>,
    static_storage: Option<PublicDisk>,
    public_storage: Option<PublicDisk>,
    private_storage: Option<PrivateDisk>,
    cdn_storage: Option<PublicDisk>,
    disable_downloads: Option<bool>,
    max_download_mb: Option<u32>,
    mods_cache: Option<Cache<IndexQueryParams, ApiResponse<PaginatedData<Mod>>>>,
    debug: Option<bool>,
    http_client: Option<reqwest::Client>,
    check_dns_http_client: Option<reqwest::Client>,
}

impl AppDataBuilder {
    fn start(pool: sqlx::postgres::PgPool) -> AppDataBuilder {
        AppDataBuilder {
            db: pool,
            app_url: None,
            front_url: None,
            port: None,
            github_client: None,
            webhook_url: None,
            index_admin_webhook_url: None,
            static_storage: None,
            public_storage: None,
            private_storage: None,
            cdn_storage: None,
            disable_downloads: None,
            max_download_mb: None,
            mods_cache: None,
            debug: None,
            http_client: None,
            check_dns_http_client: None,
        }
    }

    fn default_http_client() -> Result<reqwest::Client, reqwest::Error> {
        reqwest::Client::builder()
            .pool_max_idle_per_host(4)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .build()
    }

    fn default_check_dns_http_client() -> Result<reqwest::Client, reqwest::Error> {
        reqwest::Client::builder()
            .dns_resolver(Arc::new(ValidateDnsResolver))
            .pool_max_idle_per_host(4)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
    }

    pub fn build(self) -> anyhow::Result<AppData> {
        let app_url = self.app_url.unwrap_or("http://localhost:8080".to_owned());
        let http_client = self
            .http_client
            .map_or_else(AppDataBuilder::default_http_client, Ok)?;
        let check_dns_http_client = self
            .check_dns_http_client
            .map_or_else(AppDataBuilder::default_check_dns_http_client, Ok)?;

        Ok(AppData {
            db: self.db,
            app_url: app_url.clone(),
            front_url: self.front_url.unwrap_or("http://localhost:5173".to_owned()),
            github_client: self.github_client.unwrap_or_else(|| {
                GithubClient::new("".to_owned(), "".to_owned(), http_client.clone())
            }),
            webhook_url: self.webhook_url.unwrap_or_default(),
            index_admin_webhook_url: self.index_admin_webhook_url.unwrap_or_default(),
            static_storage: self.static_storage.unwrap_or_else(|| {
                PublicDisk::new(
                    Arc::new(LocalBackend::new("static")),
                    format!("{app_url}/static"),
                )
            }),
            public_storage: self.public_storage.unwrap_or_else(|| {
                PublicDisk::new(
                    Arc::new(LocalBackend::new("storage/public")),
                    format!("{app_url}/storage"),
                )
            }),
            private_storage: self.private_storage.unwrap_or_else(|| {
                PrivateDisk::new(Arc::new(LocalBackend::new("storage/private")))
            }),
            cdn_storage: self.cdn_storage,
            disable_downloads: self.disable_downloads.unwrap_or(false),
            max_download_mb: self.max_download_mb.unwrap_or(250),
            port: self.port.unwrap_or(8080),
            debug: self.debug.unwrap_or(false),
            mods_cache: self.mods_cache.unwrap_or_else(|| {
                Cache::builder()
                    .max_capacity(256)
                    .time_to_idle(Duration::from_mins(5))
                    .time_to_live(Duration::from_mins(10))
                    .build()
            }),
            http_client,
            check_dns_http_client,
            s3_sender: OnceLock::new(),
        })
    }

    pub fn app_url(mut self, app_url: String) -> Self {
        self.app_url = Some(app_url);
        self
    }

    pub fn front_url(mut self, front_url: String) -> Self {
        self.front_url = Some(front_url);
        self
    }

    pub fn github_client(mut self, github_client: GithubClient) -> Self {
        self.github_client = Some(github_client);
        self
    }

    pub fn webhook_url(mut self, webhook_url: String) -> Self {
        self.webhook_url = Some(webhook_url);
        self
    }

    pub fn index_admin_webhook_url(mut self, index_admin_webhook_url: String) -> Self {
        self.index_admin_webhook_url = Some(index_admin_webhook_url);
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = Some(debug);
        self
    }

    pub fn disable_downloads(mut self, disable_downloads: bool) -> Self {
        self.disable_downloads = Some(disable_downloads);
        self
    }

    pub fn max_download_mb(mut self, max_download_mb: u32) -> Self {
        self.max_download_mb = Some(max_download_mb);
        self
    }

    pub fn static_storage(mut self, static_storage: PublicDisk) -> Self {
        self.static_storage = Some(static_storage);
        self
    }

    pub fn public_storage(mut self, public_storage: PublicDisk) -> Self {
        self.public_storage = Some(public_storage);
        self
    }

    pub fn private_storage(mut self, private_storage: PrivateDisk) -> Self {
        self.private_storage = Some(private_storage);
        self
    }

    pub fn cdn_storage(mut self, cdn_storage: Option<PublicDisk>) -> Self {
        self.cdn_storage = cdn_storage;
        self
    }

    pub fn mods_cache(
        mut self,
        mods_cache: Cache<IndexQueryParams, ApiResponse<PaginatedData<Mod>>>,
    ) -> Self {
        self.mods_cache = Some(mods_cache);
        self
    }

    pub fn http_client(mut self, http_client: reqwest::Client) -> Self {
        self.http_client = Some(http_client);
        self
    }

    pub fn check_dns_http_client(mut self, check_dns_http_client: reqwest::Client) -> Self {
        self.check_dns_http_client = Some(check_dns_http_client);
        self
    }
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

    AppData::builder(pool)
        .port(dotenvy::var("PORT").map_or(3000, |x: String| x.parse::<u16>().unwrap()))
        .debug(dotenvy::var("APP_DEBUG").unwrap_or("0".to_string()) == "1")
        .app_url(dotenvy::var("APP_URL").unwrap_or("http://localhost".to_string()))

    let port =
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

    let cdn_storage = if let Some(s3_config) = S3Configuration::from_env()? {
        let backend = Arc::new(S3Backend::new(&s3_config)?);
        Some(PublicDisk::new(backend, s3_config.public_url))
    } else {
        None
    };

    let check_dns_http_client = reqwest::Client::builder()
        .dns_resolver(Arc::new(ValidateDnsResolver))
        .pool_max_idle_per_host(4)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()?;

    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(4)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .build()?;

    Ok(AppData {
        db: pool,
        app_url: app_url.clone(),
        front_url,
        github_client: GithubClient::new(github_client, github_secret, http_client.clone()),
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
        cdn_storage,
        disable_downloads,
        max_download_mb,
        port,
        debug,
        mods_cache,
        http_client,
        check_dns_http_client,
        s3_sender: OnceLock::new(),
    })
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

    pub fn github_client(&self) -> &GithubClient {
        &self.github_client
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

    pub fn cdn_storage(&self) -> Option<&PublicDisk> {
        self.cdn_storage.as_ref()
    }

    pub fn mods_cache(&self) -> &Cache<IndexQueryParams, ApiResponse<PaginatedData<Mod>>> {
        &self.mods_cache
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    /// Client that validates passed host, denies all private resolved IP addresses.
    /// Useful for preventing Server Side Request Forgery.
    ///
    /// Basically, if you have a URL as user input, *always* use this client.
    ///
    /// For an example, check mod_zip::download()
    pub fn check_dns_http_client(&self) -> &reqwest::Client {
        &self.check_dns_http_client
    }

    pub(crate) fn init_s3_sender(&self, sender: Sender<QueuedS3Task>) {
        self.s3_sender
            .set(sender)
            .expect("init_s3_sender must be called only once");
    }

    pub(crate) fn send_s3_task(&self, task: S3WorkerTask) -> bool {
        let Some(sender) = self.s3_sender.get() else {
            return false;
        };
        let request_id = http::REQUEST_ID.try_with(|id| *id).ok();
        sender.try_send(QueuedS3Task { task, request_id }).is_ok()
    }
}
