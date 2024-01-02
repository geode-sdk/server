use std::{fmt::Display, io::Write};
use futures::StreamExt;
use actix_web::{get, web, App, HttpServer, Responder, ResponseError, post};
use serde::Serialize;

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
    id: String,
    name: String,
    developer: String,
    download_url: Option<String>,
}

#[get("/v1/mods")]
async fn list_mods(data: web::Data<AppData>) -> Result<impl Responder, Error> {
    let mut pool = data.db.acquire().await.or(Err(Error::DbAcquireError))?;
    let mods = sqlx::query_as!(Mod, "SELECT * FROM mods")
        .fetch_all(&mut *pool)
        .await.or(Err(Error::DbError))?;

    Ok(web::Json(mods))
}

#[get("/v1/mods/{id}")]
async fn get_mod_by_id(id: String, data: web::Data<AppData>) -> Result<impl Responder, Error> {
    let mut pool = data.db.acquire().await.or(Err(Error::DbAcquireError))?;
    let res = sqlx::query_as!(Mod, r#"SELECT * FROM mods WHERE id = ?"#, id)
        .fetch_one(&mut *pool)
        .await.or(Err(Error::DbError))?;

    Ok(web::Json(res))
}

#[post("/v1/mods/{id}")]
async fn publish_mod(id: String, data: web::Data<AppData>, mut geode_file: web::Payload) -> Result<impl Responder, Error> {
    // todo: authenticate
    let mut file = std::fs::File::open(format!("db/temp_{id}.geode")).or(Err(Error::FsError))?;
    //                                                   ^ todo: sanitize
    let mut written = 0usize;
    while let Some(chunk) = geode_file.next().await {
        let chunk = chunk.map_err(|e| Error::UploadError(e.to_string()))?;
        written += chunk.len();
        if written > 262_144 {
            return Err(Error::UploadError("file too large".to_string()));
        }
        file.write_all(&chunk).or(Err(Error::FsError))?;
    }

    // todo: load info from geode file and add to database

    Ok(web::Json(None::<()>))
}

#[get("/")]
async fn hello_index() -> Result<impl Responder, Error> {
    Ok("Hi! :D")
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

    Ok(
        HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(AppData { db: pool.clone() }))
                .service(list_mods)
                .service(get_mod_by_id)
                .service(publish_mod)
                .service(hello_index)
        })
            .bind(("127.0.0.1", dotenvy::var("PORT").map_or(8080, |x| x.parse::<u16>().unwrap())))?
            .run()
            .await?
    )
}
