use actix_web::{get, web, Responder, post, HttpResponse, patch};
use serde::Deserialize;
use sqlx::Acquire;
use log::info;

use crate::types::api::{ApiError, ApiResponse};
use crate::AppData;
use crate::types::models::mod_entity::{Mod, download_geode_file};
use crate::types::mod_json::ModJson;
use crate::types::models::mod_gd_version::GDVersionEnum;

#[derive(Deserialize)]
pub struct IndexQueryParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub query: Option<String>,
    pub gd: GDVersionEnum
}

#[derive(Deserialize)]
struct CreateQueryParams {
    download_url: String
}

#[get("/v1/mods")]
pub async fn index(data: web::Data<AppData>, query: web::Query<IndexQueryParams>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let result = Mod::get_index(&mut pool, query.0).await?;
    Ok(web::Json(ApiResponse {error: "".into(), data: result}))
}

#[get("/v1/mods/{id}")]
pub async fn get(id: String, data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let found = Mod::get_one(&id, &mut pool).await?;
    match found {
        Some(m) => Ok(web::Json(ApiResponse {error: "".into(), data: m})),
        None => Err(ApiError::NotFound("".into()))
    }
}

#[post("/v1/mods")]
pub async fn create(data: web::Data<AppData>, payload: web::Json<CreateQueryParams>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let file_path = download_geode_file(&payload.download_url).await?;
    let json = ModJson::from_zip(&file_path, payload.download_url.as_str())?;
    let mut transaction = pool.begin().await.or(Err(ApiError::DbError))?;
    let result = Mod::from_json(&json, &mut transaction).await;
    if result.is_err() {
        let _ = transaction.rollback().await;
        let _ = tokio::fs::remove_file(file_path).await;
        return Err(result.err().unwrap());
    }
    let tr_res = transaction.commit().await;
    if tr_res.is_err() {
        info!("{:?}", tr_res);
    }
    let _ = tokio::fs::remove_file(file_path).await;
    Ok(HttpResponse::NoContent())
}

#[patch("/v1/mods")]
pub async fn update(data: web::Data<AppData>, payload: web::Json<CreateQueryParams>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let file_path = download_geode_file(&payload.download_url).await?;
    let json = ModJson::from_zip(&file_path, payload.download_url.as_str()).or(Err(ApiError::FilesystemError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::DbError))?;
    let result = Mod::new_version(&json, &mut transaction).await;
    if result.is_err() {
        let _ = transaction.rollback().await;
        let _ = tokio::fs::remove_file(file_path).await;
        return Err(result.err().unwrap());
    }
    let _ = transaction.commit().await;
    let _ = tokio::fs::remove_file(file_path).await;
    Ok(HttpResponse::NoContent())
}