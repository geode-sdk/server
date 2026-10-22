use actix_web::web;

mod abbreviate;
mod auth;
mod cli;
mod config;
mod database;
mod dns;
mod events;
mod extractors;
mod http;
mod jobs;
mod logging;
mod mod_zip;
mod openapi;
mod s3_worker;
mod storage;
mod types;
mod webhook;

// Avoid musl's default allocator due to lackluster performance
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init();

    let app_data = config::build_config().await?;
    app_data.static_storage().init().await?;
    app_data.private_storage().init().await?;
    if let Some(cdn_storage) = app_data.cdn_storage() {
        cdn_storage.init().await?;
    }

    if cli::maybe_cli(&app_data).await? {
        return Ok(());
    }

    tracing::info!("Running migrations");
    sqlx::migrate!("./migrations").run(app_data.db()).await?;

    let data = web::Data::new(app_data);
    tokio::spawn(s3_worker::run_s3_worker(data.clone()));

    http::serve_http(data).await
}
