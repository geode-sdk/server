use actix_web::{get, web, Responder, post};
use serde::Deserialize;
use sqlx::Acquire;

use crate::types::api::ApiError;
use crate::AppData;
use crate::types::models::mod_entity::{Mod, download_geode_file};
use crate::types::mod_json::ModJson;

#[derive(Deserialize)]
pub struct IndexQueryParams {
    page: Option<i64>,
    per_page: Option<i64>,
    query: Option<String>
}

#[derive(Deserialize)]
struct CreateQueryParams {
    download_url: String
}

#[get("/v1/mods")]
pub async fn index(data: web::Data<AppData>, query: web::Query<IndexQueryParams>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(10);
    let query = query.query.clone().unwrap_or("".to_string());

    let result = Mod::get_index(&mut pool, page, per_page, query).await?;
    Ok(web::Json(result))
}

#[get("/v1/mods/{id}")]
pub async fn get(id: String, data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    // let pool = data.db.acquire().await.or(Err(Error::DbAcquireError))?;
    // let res = sqlx::query_as!(Mod, r#"SELECT * FROM mods WHERE id = $1"#, id)
    //     .fetch_one(&mut *pool)
    //     .await.or(Err(Error::DbError))?;

    Ok(web::Json(""))
}

#[post("/v1/mods")]
pub async fn create(data: web::Data<AppData>, payload: web::Json<CreateQueryParams>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let file_path = download_geode_file(&payload.download_url).await?;
    let json = ModJson::from_zip(&file_path).or(Err(ApiError::FilesystemError))?;
    pool.begin();
    Mod::from_json(&json, true, &mut pool).await?;
    _ = tokio::fs::remove_file(file_path);

    // // todo: authenticate
    // let mut file = std::fs::File::open(format!("db/temp_{id}.geode")).or(Err(Error::FsError))?;
    // //                                                   ^ todo: sanitize
    // let mut written = 0usize;
    // while let Some(chunk) = geode_file.next().await {
    //     let chunk = chunk.map_err(|e| Error::UploadError(e.to_string()))?;
    //     written += chunk.len();
    //     if written > 262_144 {
    //         return Err(Error::UploadError("file too large".to_string()));
    //     }
    //     file.write_all(&chunk).or(Err(Error::FsError))?;
    // }

    

    // todo: load info from geode file and add to database

    Ok(web::Json(None::<()>))
}
