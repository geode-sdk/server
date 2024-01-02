use actix_web::{get, web, App, HttpServer, Responder, Error};
use serde::Serialize;

struct AppData {
    db: sqlx::SqlitePool,
}

#[derive(Serialize)]
struct Mod {
    id: String,
    name: String,
    developer: String,
}

#[get("/v1/mods")]
async fn list_mods(data: web::Data<AppData>) -> Result<impl Responder, Error> {
    let mut pool = data.db.acquire().await?;
    let mods = sqlx::query_as!(Mod, "SELECT * FROM mods")
        .fetch_all(&mut *pool)
        .await.map_err(|e| e.into())?;

    Ok(web::Json(mods))
}

#[get("/v1/mods/{id}")]
async fn get_mod_by_id(id: String, data: web::Data<AppData>) -> Result<impl Responder, Error> {
    let mut pool = data.db.acquire().await.map_err(|e| e.into())?;
    let res = sqlx::query_as!(Mod, r#"SELECT * FROM mods WHERE id = ?"#, id)
        .fetch_one(&mut *pool)
        .await.map_err(|e| e.into())?;
    Ok("")
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenvy::dotenv()?;

    // Set up logger
    // env_logger::init();

    // Connect to the index database
    let pool = sqlx::SqlitePool::connect("../db/geode-index.db").await?;

    Ok(
        HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(AppData { db: pool.clone() }))
                .service(list_mods)
                .service(get_mod_by_id)
        })
            .bind(("127.0.0.1", 8080))?
            .run()
            .await?
    )
}
