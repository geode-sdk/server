use actix_web::{get, web, App, HttpServer, Responder, middleware::Logger};
use log::info;
use env_logger::Env;

use crate::types::api::ApiError;

mod endpoints;
mod types;

pub struct AppData {
    db: sqlx::postgres::PgPool,
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

    info!("Starting server on {}:{}", addr, port);
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppData { db: pool.clone() }))
            .wrap(Logger::default())
            .service(endpoints::mods::index)
            .service(endpoints::mods::get)
            .service(endpoints::mods::create)
            .service(endpoints::mods::update)
            .service(health)
    })
        .bind((addr, port))?
        .run()
        .await?;
    anyhow::Ok(())
}
