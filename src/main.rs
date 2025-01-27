use actix_cors::Cors;
use actix_web::{
    middleware::Logger,
    web::{self, QueryConfig},
    App, HttpServer,
};
use endpoints::mods::{IndexQueryParams, IndexSortType};
use forum::discord::{create_or_update_thread, get_threads};
use types::models::{mod_entity::Mod, mod_version::ModVersion, mod_version_status::ModVersionStatusEnum};

use crate::types::api;

mod auth;
mod cli;
mod config;
mod database;
mod endpoints;
mod events;
mod extractors;
mod jobs;
mod types;
mod forum;
mod webhook;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("config/log4rs.yaml", Default::default())
        .map_err(|e| e.context("Failed to read log4rs config"))?;

    let app_data = config::build_config().await?;

    if cli::maybe_cli(&app_data).await? {
        return Ok(());
    }

    log::info!("Running migrations");
    sqlx::migrate!("./migrations").run(app_data.db()).await?;

    let port = app_data.port();
    let debug = app_data.debug();
    let data = app_data.clone();

    log::info!("Starting server on 0.0.0.0:{}", port);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
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
            .service(endpoints::auth::github::start_github_web_login)
            .service(endpoints::auth::refresh_token)
            .service(endpoints::auth::github::github_web_callback)
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
        if !app_data.discord().is_valid() {
            log::error!("Discord configuration is not set up. Not creating forum threads.");
            return;
        }

        log::info!("Starting forum thread creation job");
        let pool_res = app_data.db().acquire().await;
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

        let threads = get_threads(&app_data.discord()).await;
        let threads_res = Some(threads);
        let mut mods = results.unwrap().data;
        mods.sort_by(|a, b| {
            let a = &a.versions[0];
            let b = &b.versions[0];
            a.created_at.cmp(&b.created_at)
        });
        for i in 0..mods.len() {
            let m = &mods[i];
            let version_res = ModVersion::get_one(&m.id, &m.versions[0].version, true, false, &mut pool).await;
            if version_res.is_err() {
                continue;
            }

            if i != 0 && i % 10 == 0 {
                log::info!("Created {i} threads, sleeping for 10 seconds");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }

            create_or_update_thread(
                threads_res.clone(),
                &app_data.discord(),
                m,
                &version_res.unwrap(),
                "",
                &app_data.app_url()
            ).await;
        }

        log::info!("Finished creating forum threads");
    });

    if debug {
        log::info!("Running in debug mode, using 1 thread.");
        server.workers(1).run().await?;
    } else {
        server.run().await?;
    }

    anyhow::Ok(())
}
