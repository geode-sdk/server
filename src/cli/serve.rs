use actix_web::web;

use crate::{config::AppData, database, http, s3_worker};

#[derive(clap::Args, Debug, Default)]
pub struct ServeArgs {
    /// Don't run database migrations on startup
    #[arg(long, env = "SKIP_MIGRATIONS", value_parser = clap::builder::FalseyValueParser::new())]
    skip_migrations: bool,
}

pub async fn run(args: ServeArgs, data: AppData) -> anyhow::Result<()> {
    data.static_storage().init().await?;
    data.private_storage().init().await?;

    if let Some(cdn) = data.cdn_storage() {
        cdn.init().await?;
    }

    if !args.skip_migrations {
        database::migrate(data.db()).await?;
    }

    let data = web::Data::new(data);
    tokio::spawn(s3_worker::run_s3_worker(data.clone()));
    http::serve_http(data).await
}
