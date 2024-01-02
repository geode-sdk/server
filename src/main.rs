use std::fmt::Display;
use actix_web::{get, web, App, HttpServer, Responder, ResponseError};
use serde::Serialize;

mod endpoints;
mod types;
struct AppData {
    db: sqlx::SqlitePool,
}

#[derive(Debug)]
enum Error {
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

#[derive(Serialize)]
struct Mod {
    id: Option<String>,
    name: Option<String>,
    developer: Option<String>,
    download_url: Option<String>,
}

#[get("/")]
async fn health() -> Result<impl Responder, Error> {
    Ok(web::Json("The Geode Index is running"))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenvy::dotenv()?;

    // Set up logger
    // env_logger::init();

    let db_url = dotenvy::var("DATABASE_URL")?;

    // Connect to the index database
    let pool = sqlx::SqlitePool::connect(&db_url).await?;
    let addr = "127.0.0.1";
    let port = dotenvy::var("PORT").map_or(8080, |x: String| x.parse::<u16>().unwrap());

    println!("Starting server on {}:{}", addr, port);
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppData { db: pool.clone() }))
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
