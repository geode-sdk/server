use std::io::Write;

use actix_web::{get, web, Responder, post};
use futures::StreamExt;
use serde::Deserialize;

use crate::{AppData, Error};
use crate::types::models::mod_entity::Mod;

#[derive(Deserialize)]
struct IndexQueryParams {
    page: Option<i64>,
    per_page: Option<i64>,
}

#[get("/v1/mods")]
pub async fn index(data: web::Data<AppData>, query: web::Query<IndexQueryParams>) -> Result<impl Responder, Error> {
    let mut pool = data.db.acquire().await.or(Err(Error::DbAcquireError))?;
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(10);

    let result = Mod::get_index(&mut pool, page, per_page).await?;
    Ok(web::Json(result))
}

#[get("/v1/mods/{id}")]
pub async fn get(id: String, data: web::Data<AppData>) -> Result<impl Responder, Error> {
    // let pool = data.db.acquire().await.or(Err(Error::DbAcquireError))?;
    // let res = sqlx::query_as!(Mod, r#"SELECT * FROM mods WHERE id = $1"#, id)
    //     .fetch_one(&mut *pool)
    //     .await.or(Err(Error::DbError))?;

    Ok(web::Json(""))
}

#[post("/v1/mods/{id}")]
pub async fn create(id: String, data: web::Data<AppData>, mut geode_file: web::Payload) -> Result<impl Responder, Error> {
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
