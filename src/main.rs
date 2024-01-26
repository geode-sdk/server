use actix_web::{get, web::{self, QueryConfig}, App, HttpServer, Responder, middleware::Logger};
use log::info;
use env_logger::Env;

use crate::types::api::ApiError;
use crate::types::api;

mod endpoints;
mod types;

pub struct AppData {
    db: sqlx::postgres::PgPool,
    debug: bool,
    app_url: String
}

#[get("/")]
async fn health() -> Result<impl Responder, ApiError> {
    Ok(web::Json("The Geode Index is running"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let env_url = dotenvy::var("DATABASE_URL")?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect(&env_url).await?;
    let addr = "127.0.0.1";
    let port = dotenvy::var("PORT").map_or(8080, |x: String| x.parse::<u16>().unwrap());
    let debug = dotenvy::var("APP_DEBUG").unwrap_or("0".to_string()) == "1";
    let app_url = dotenvy::var("APP_URL").unwrap_or("http://localhost".to_string());

    info!("Starting server on {}:{}", addr, port);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppData { db: pool.clone(), debug, app_url: app_url.clone() }))
            .app_data(QueryConfig::default().error_handler(api::query_error_handler))
            .wrap(Logger::default())
            .service(endpoints::mods::index)
            .service(endpoints::mods::get)
            .service(endpoints::mods::create)
            .service(endpoints::mod_versions::get_one)
            .service(endpoints::mod_versions::download_version)
            .service(endpoints::mod_versions::create_version)
            .service(endpoints::auth::github::poll_github_login)
            .service(endpoints::auth::github::start_github_login)
            .service(health)
    }).bind((addr, port))?;

    if debug {
        info!("Running in debug mode, using 1 thread.");
        server.workers(1).run().await?;
    } else {
        server.run().await?;
    }

    anyhow::Ok(())
}
