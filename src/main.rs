use actix_cors::Cors;
use actix_web::{
    middleware::Logger,
    web::{self, QueryConfig},
    App, HttpServer
};
use clap::Parser;
use endpoints::mods::{IndexQueryParams, IndexSortType};
use forum::{create_or_update_thread, get_threads};
use types::models::{mod_entity::Mod, mod_version::ModVersion, mod_version_status::ModVersionStatusEnum};

use crate::types::api;

mod auth;
mod cli;
mod database;
mod endpoints;
mod events;
mod extractors;
mod jobs;
mod types;
mod forum;
mod webhook;

#[derive(Clone)]
pub struct AppData {
    db: sqlx::postgres::PgPool,
    app_url: String,
    github_client_id: String,
    github_client_secret: String,
    webhook_url: String,
    bot_token: String,
    guild_id: u64,
    channel_id: u64,
    disable_downloads: bool,
    max_download_mb: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("config/log4rs.yaml", Default::default())
        .map_err(|e| e.context("Failed to read log4rs config"))?;

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
    let bot_token = dotenvy::var("DISCORD_BOT_TOKEN").unwrap_or("".to_string());
    let guild_id = dotenvy::var("DISCORD_GUILD_ID").unwrap_or("0".to_string()).parse::<u64>().unwrap_or(0);
    let channel_id = dotenvy::var("DISCORD_CHANNEL_ID").unwrap_or("0".to_string()).parse::<u64>().unwrap_or(0);
    let disable_downloads = dotenvy::var("DISABLE_DOWNLOAD_COUNTS").unwrap_or("0".to_string()) == "1";
    let max_downloadmb = dotenvy::var("MAX_MOD_FILESIZE_MB")
        .unwrap_or("250".to_string())
        .parse::<u32>()
        .unwrap_or(250);

    let app_data = AppData {
        db: pool.clone(),
        app_url: app_url.clone(),
        github_client_id: github_client,
        github_client_secret: github_secret,
        webhook_url,
        bot_token: bot_token.clone(),
        guild_id,
        channel_id,
        disable_downloads,
        max_download_mb: max_downloadmb,
    };

    if cli::maybe_cli(&app_data).await? {
        return Ok(());
    }

    log::info!("Running migrations");
    let migration_res = sqlx::migrate!("./migrations").run(&app_data.db).await;
    if migration_res.is_err() {
        log::error!(
            "Error encountered while running migrations: {}",
            migration_res.err().unwrap()
        );
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
            .service(endpoints::auth::github::github_token_login)
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

    tokio::spawn(async move {
        if guild_id == 0 || channel_id == 0 || bot_token.is_empty() {
            log::error!("Discord configuration is not set up. Not creating forum threads.");
            return;
        }

        log::info!("Starting forum thread creation job");
        let pool_res = pool.clone().acquire().await;
        if pool_res.is_err() {
            return;
        }
        let mut pool = pool_res.unwrap();
        let query = IndexQueryParams {
            page: None,
            per_page: Some(100),
            query: None,
            gd: None,
            platforms: None,
            sort: IndexSortType::Downloads,
            geode: None,
            developer: None,
            tags: None,
            featured: None,
            status: Some(ModVersionStatusEnum::Pending),
        };
        let results = Mod::get_index(&mut pool, query).await;
        if results.is_err() {
            return;
        }

        let threads = get_threads(guild_id, channel_id, bot_token.clone()).await;
        let threads_res = Some(threads);
        let mut i = 0;
        for m in results.unwrap().data {
            let v_res = ModVersion::get_one(&m.id, &m.versions[0].version, true, false, &mut pool).await;
            if v_res.is_err() {
                i += 1;
                continue;
            }

            let v = v_res.unwrap();

            if i != 0 && i % 10 == 0 {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }

            log::info!("Creating thread for mod {}", m.id);

            create_or_update_thread(
                threads_res.clone(),
                guild_id,
                channel_id,
                bot_token.clone(),
                m,
                v,
                None,
                app_url.clone()
            ).await;

            i += 1;
        }
    });

    if debug {
        log::info!("Running in debug mode, using 1 thread.");
        server.workers(1).run().await?;
    } else {
        server.run().await?;
    }

    anyhow::Ok(())
}
