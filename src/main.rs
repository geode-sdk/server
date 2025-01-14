use crate::types::api;
use actix_cors::Cors;
use actix_web::{
    middleware::Logger,
    web::{self, QueryConfig},
    App, HttpServer
};

mod auth;
mod endpoints;
mod extractors;
mod jobs;
mod types;
mod webhook;
mod cli;
mod database;

#[derive(Clone)]
pub struct AppData {
    db: sqlx::postgres::PgPool,
    app_url: String,
    github_client_id: String,
    github_client_secret: String,
    webhook_url: String,
    disable_downloads: bool,
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("config/log4rs.yaml", Default::default())?;

    let env_url = dotenvy::var("DATABASE_URL")?;

    let pool = sqlx::postgres::PgPoolOptions::default()
        .max_connections(10)
        .connect(&env_url)
        .await?;

    let port = dotenvy::var("PORT").map_or(8080, |x: String| x.parse::<u16>().unwrap());
    let debug = dotenvy::var("APP_DEBUG").unwrap_or("0".to_string()) == "1";
    let app_url = dotenvy::var("APP_URL").unwrap_or("http://localhost".to_string());
    let github_client = dotenvy::var("GITHUB_CLIENT_ID").unwrap_or("".to_string());
    let github_secret = dotenvy::var("GITHUB_CLIENT_SECRET").unwrap_or("".to_string());
    let webhook_url = dotenvy::var("DISCORD_WEBHOOK_URL").unwrap_or("".to_string());
    let disable_downloads =
        dotenvy::var("DISABLE_DOWNLOAD_COUNTS").unwrap_or("0".to_string()) == "1";

    let app_data = AppData {
        db: pool,
        app_url,
        github_client_id: github_client,
        github_client_secret: github_secret,
        webhook_url,
        disable_downloads,
    };

    if cli::maybe_cli(&app_data).await? {
        return Ok(());
    }

    log::info!("Running migrations");
    if let Err(e) = sqlx::migrate!("./migrations").run(&app_data.db).await {
        log::error!("Error encountered while running migrations: {}", e);
    }

    log::info!("Starting server on 0.0.0.0:{}", port);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_data.clone()))
            .app_data(QueryConfig::default().error_handler(api::query_error_handler))
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"])
                    .allow_any_header()
                    .supports_credentials()
                    .max_age(3600),
            )
            .wrap(Logger::default())
            .service(endpoints::mods::index)
            .service(endpoints::mods::get_mod_updates)
            .service(endpoints::mods::get)
            .service(endpoints::mods::create)
            .service(endpoints::mods::update_mod)
            .service(endpoints::mods::get_logo)
            .service(endpoints::mod_versions::get_version_index)
            .service(endpoints::mod_versions::get_one)
            .service(endpoints::mod_versions::download_version)
            .service(endpoints::mod_versions::create_version)
            .service(endpoints::mod_versions::update_version)
            .service(endpoints::auth::github::poll_github_login)
            .service(endpoints::auth::github::start_github_login)
            .service(endpoints::developers::developer_index)
            .service(endpoints::developers::get_developer)
            .service(endpoints::developers::add_developer_to_mod)
            .service(endpoints::developers::remove_dev_from_mod)
            .service(endpoints::developers::delete_token)
            .service(endpoints::developers::delete_tokens)
            .service(endpoints::developers::update_profile)
            .service(endpoints::developers::get_own_mods)
            .service(endpoints::developers::get_me)
            .service(endpoints::developers::update_developer)
            .service(endpoints::tags::index)
            .service(endpoints::tags::detailed_index)
            .service(endpoints::stats::get_stats)
            .service(endpoints::health::health)
    })
    .bind(("0.0.0.0", port))?;

    if debug {
        log::info!("Running in debug mode, using 1 thread.");
        server.workers(1).run().await?;
    } else {
        server.run().await?;
    }

    anyhow::Ok(())
}

