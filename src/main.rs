use std::fmt::Display;
use actix_web::{get, web, App, HttpServer, Responder, ResponseError, middleware::Logger};
use log::info;
use env_logger::Env;

mod endpoints;
mod types;

pub struct AppData {
    db: sqlx::postgres::PgPool,
}

#[derive(Debug)]
pub enum Error {
    FsError,
    DbAcquireError,
    DbError,
    UploadError(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FsError => write!(f, "server filesystem error"),
            Self::DbAcquireError => write!(f, "database busy"),
            Self::DbError => write!(f, "database error"),
            Self::UploadError(msg) => write!(f, "upload error: {msg}"),
        }
    }
}

impl ResponseError for Error {}

#[get("/")]
async fn health() -> Result<impl Responder, Error> {
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
            .service(health)
    })
        .bind((addr, port))?
        .run()
        .await?;
    anyhow::Ok(())
}
