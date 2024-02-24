use actix_web::{
    get,
    middleware::Logger,
    web::{self, QueryConfig},
    App, HttpServer, Responder,
};
use clap::Parser;
use env_logger::Env;
use log::info;

use crate::types::api;
use crate::types::api::ApiError;

mod auth;
mod endpoints;
mod extractors;
mod jobs;
mod types;

#[derive(Clone)]
pub struct AppData {
    db: sqlx::postgres::PgPool,
    debug: bool,
    app_url: String,
    github_client_id: String,
    github_client_secret: String,
}

#[derive(Debug, Parser)]
struct Args {
    /// Name of the script to run
    #[arg(short, long)]
    script: Option<String>,
}

#[get("/")]
async fn health() -> Result<impl Responder, ApiError> {
    Ok(web::Json("The Geode Index is running"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let env_url = dotenvy::var("DATABASE_URL")?;

    let pool = sqlx::postgres::PgPoolOptions::default()
        .max_connections(10)
        .connect(&env_url)
        .await?;
    info!("Running migrations");
    let migration_res = sqlx::migrate!("./migrations").run(&pool).await;
    if migration_res.is_err() {
        log::error!(
            "Error encountered while running migrations: {}",
            migration_res.err().unwrap()
        );
    }
    let addr = "0.0.0.0";
    let port = dotenvy::var("PORT").map_or(8080, |x: String| x.parse::<u16>().unwrap());
    let debug = dotenvy::var("APP_DEBUG").unwrap_or("0".to_string()) == "1";
    let app_url = dotenvy::var("APP_URL").unwrap_or("http://localhost".to_string());
    let github_client = dotenvy::var("GITHUB_CLIENT_ID").unwrap_or("".to_string());
    let github_secret = dotenvy::var("GITHUB_CLIENT_SECRET").unwrap_or("".to_string());

    let app_data = AppData {
        db: pool.clone(),
        debug,
        app_url: app_url.clone(),
        github_client_id: github_client.clone(),
        github_client_secret: github_secret.clone(),
    };

    let args = Args::parse();
    if let Some(s) = args.script {
        if let Err(e) = jobs::start_job(&s, app_data).await {
            log::error!("Error encountered while running job: {}", e);
        }
        log::info!("Job {} completed", s);
        return anyhow::Ok(());
    }

    info!("Starting server on {}:{}", addr, port);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_data.clone()))
            .app_data(QueryConfig::default().error_handler(api::query_error_handler))
            .wrap(Logger::default())
            .service(endpoints::mods::index)
            .service(endpoints::mods::get)
            .service(endpoints::mods::create)
            .service(endpoints::mods::update_mod)
            .service(endpoints::mods::get_logo)
            .service(endpoints::mod_versions::get_one)
            .service(endpoints::mod_versions::download_version)
            .service(endpoints::mod_versions::create_version)
            .service(endpoints::mod_versions::update_version)
            .service(endpoints::auth::github::poll_github_login)
            .service(endpoints::auth::github::start_github_login)
            .service(endpoints::developers::add_developer_to_mod)
            .service(endpoints::developers::remove_dev_from_mod)
            .service(health)
    })
    .bind((addr, port))?;

    if debug {
        info!("Running in debug mode, using 1 thread.");
        server.workers(1).run().await?;
    } else {
        server.run().await?;
    }

    anyhow::Ok(())
}
