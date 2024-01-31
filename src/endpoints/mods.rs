use actix_web::{get, web, Responder, post, HttpResponse};
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
    #[serde(default)]
    pub gd: Option<GDVersionEnum>,
    #[serde(default)]
    pub platforms: Option<String>
}

#[derive(Deserialize)]
struct CreateQueryParams {
    download_url: String
}

#[get("/v1/mods")]
pub async fn index(data: web::Data<AppData>, query: web::Query<IndexQueryParams>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let mut result = Mod::get_index(&mut pool, query.0).await?;
    for i in &mut result.data {
        for j in &mut i.versions {
            j.modify_download_link(&data.app_url);
        }
    }
    Ok(web::Json(ApiResponse {error: "".into(), payload: result}))
}

#[get("/v1/mods/{id}")]
pub async fn get(data: web::Data<AppData>, id: web::Path<String>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let found = Mod::get_one(&id, &mut pool).await?;
    match found {
        Some(mut m) => {
            for i in &mut m.versions {
                i.modify_download_link(&data.app_url);
            }
            Ok(web::Json(ApiResponse {error: "".into(), payload: m}))
        },
        None => Err(ApiError::NotFound("".into()))
    }
}

#[post("/v1/mods")]
pub async fn create(data: web::Data<AppData>, payload: web::Json<CreateQueryParams>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut file_path = download_geode_file(&payload.download_url).await?;
    let json = ModJson::from_zip(&mut file_path, payload.download_url.as_str())?;
    let mut transaction = pool.begin().await.or(Err(ApiError::DbError))?;
    let result = Mod::from_json(&json, &mut transaction).await;
    if result.is_err() {
        let _ = transaction.rollback().await;
        return Err(result.err().unwrap());
    }
    let tr_res = transaction.commit().await;
    if tr_res.is_err() {
        info!("{:?}", tr_res);
    }
    Ok(HttpResponse::NoContent())
}