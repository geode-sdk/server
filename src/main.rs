use clap::Parser;

mod abbreviate;
mod auth;
mod cli;
mod config;
mod database;
mod dns;
mod events;
mod http;
mod jobs;
mod logging;
mod mod_zip;
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
    let args = cli::Args::parse();
    logging::init();
    let app_data = config::build_config().await?;
    cli::run(args, app_data).await
}
