use actix_web::{get, web, Responder};
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
    let version = ModVersion::get_one(&path.id, &path.version, &mut pool).await?;
    Ok(web::Json(ApiResponse {error: "".to_string(), data: version}))
}