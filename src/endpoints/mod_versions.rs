use actix_web::{get, post, web, Responder, HttpResponse};
use serde::Deserialize;
use sqlx::Acquire;

use crate::{AppData, types::{api::{ApiError, ApiResponse}, models::{mod_version::ModVersion, mod_entity::{download_geode_file, Mod}}, mod_json::ModJson}};

#[derive(Deserialize)]
pub struct GetOnePath {
    id: String,
    version: String
}

#[derive(Deserialize)]
pub struct CreateQueryParams {
    download_url: String
}

#[derive(Deserialize)]
pub struct CreateVersionPath {
    id: String
}

#[get("v1/mods/{id}/versions/{version}")]
pub async fn get_one(path: web::Path<GetOnePath>, data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut version = ModVersion::get_one(&path.id, &path.version, &mut pool).await?;
    version.modify_download_link(&data.app_url);
    Ok(web::Json(ApiResponse {error: "".to_string(), payload: version}))
}

#[get("v1/mods/{id}/versions/{version}/download")]
pub async fn download_version(path: web::Path<GetOnePath>, data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let url = ModVersion::get_download_url(&path.id, &path.version, &mut pool).await?;
    Ok(HttpResponse::Found().append_header(("Location", url)).finish())
}

#[post("v1/mods/{id}/versions")]
pub async fn create_version(path: web::Path<CreateVersionPath>, data: web::Data<AppData>, payload: web::Json<CreateQueryParams >) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut file_path = download_geode_file(&payload.download_url).await?;
    let json = ModJson::from_zip(&mut file_path, payload.download_url.as_str()).or(Err(ApiError::FilesystemError))?;
    if json.id != path.id {
        return Err(ApiError::BadRequest(format!("Request id {} does not match mod.json id {}", path.id, json.id)));
    }
    let mut transaction = pool.begin().await.or(Err(ApiError::DbError))?;
    let result = Mod::new_version(&json, &mut transaction).await;
    if result.is_err() {
        let _ = transaction.rollback().await;
        return Err(result.err().unwrap());
    }
    let _ = transaction.commit().await;
    Ok(HttpResponse::NoContent())
}