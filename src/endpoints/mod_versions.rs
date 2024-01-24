use actix_web::{get, web, Responder, HttpResponse};
use serde::Deserialize;

use crate::{AppData, types::{api::{ApiError, ApiResponse}, models::mod_version::ModVersion}};

#[derive(Deserialize)]
struct GetOnePath {
    id: String,
    version: String
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