use std::io::Write;

use actix_web::{get, web, Responder, post};
use futures::StreamExt;
use serde::Deserialize;

use crate::types::api::PaginatedData;
use crate::{AppData, Error};
use crate::types::models::Mod;

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

    let limit = per_page;
    let offset = (page - 1) * per_page;

    let mods = sqlx::query_as!(Mod, r#"SELECT * FROM mods LIMIT $1 OFFSET $2"#, limit, offset)
        .fetch_all(&mut *pool)
        .await.or(Err(Error::DbError))?;

    let count: Option<i64> = sqlx::query_scalar!("SELECT COUNT(*) as count FROM mods").fetch_one(&mut *pool).await.or(Err(Error::DbError))?;
    Ok(web::Json(PaginatedData{ data: mods, count: count.unwrap_or(0) }))
}

#[get("/v1/mods/{id}")]
pub async fn get(id: String, data: web::Data<AppData>) -> Result<impl Responder, Error> {
    let mut pool = data.db.acquire().await.or(Err(Error::DbAcquireError))?;
    let res = sqlx::query_as!(Mod, r#"SELECT * FROM mods WHERE id = $1"#, id)
        .fetch_one(&mut *pool)
        .await.or(Err(Error::DbError))?;

    Ok(web::Json(res))
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
