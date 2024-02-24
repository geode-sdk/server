use actix_web::{get, post, put, web, HttpResponse, Responder};
use log::info;
use serde::{Deserialize, Serialize};
use sqlx::{Acquire, Connection};

use crate::extractors::auth::Auth;
use crate::types::api::{ApiError, ApiResponse};
use crate::types::mod_json::ModJson;
use crate::types::models::mod_entity::{download_geode_file, Mod};
use crate::types::models::mod_gd_version::GDVersionEnum;
use crate::AppData;

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndexSortType {
    #[default]
    Downloads,
    RecentlyUpdated,
    RecentlyPublished,
}

#[derive(Deserialize)]
pub struct IndexQueryParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub query: Option<String>,
    #[serde(default)]
    pub gd: Option<GDVersionEnum>,
    #[serde(default)]
    pub platforms: Option<String>,
    #[serde(default)]
    pub sort: IndexSortType,
    pub developer: Option<String>,
    pub tags: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateQueryParams {
    download_url: String,
}

#[get("/v1/mods")]
pub async fn index(
    data: web::Data<AppData>,
    query: web::Query<IndexQueryParams>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let mut result = Mod::get_index(&mut pool, query.0).await?;
    for i in &mut result.data {
        for j in &mut i.versions {
            j.modify_download_link(&data.app_url);
        }
    }
    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: result,
    }))
}

#[get("/v1/mods/{id}")]
pub async fn get(
    data: web::Data<AppData>,
    id: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let found = Mod::get_one(&id, &mut pool).await?;
    match found {
        Some(mut m) => {
            for i in &mut m.versions {
                i.modify_download_link(&data.app_url);
            }
            Ok(web::Json(ApiResponse {
                error: "".into(),
                payload: m,
            }))
        }
        None => Err(ApiError::NotFound("".into())),
    }
}

#[post("/v1/mods")]
pub async fn create(
    data: web::Data<AppData>,
    payload: web::Json<CreateQueryParams>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.into_developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut file_path = download_geode_file(&payload.download_url).await?;
    let json = ModJson::from_zip(&mut file_path, payload.download_url.as_str())?;
    let mut transaction = pool.begin().await.or(Err(ApiError::DbError))?;
    let result = Mod::from_json(&json, dev, &mut transaction).await;
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

#[get("/v1/mods/{id}/logo")]
pub async fn get_logo(
    data: web::Data<AppData>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let image = Mod::get_logo_for_mod(&path, &mut pool).await?;

    match image {
        Some(i) => Ok(HttpResponse::Ok().content_type("image/png").body(i)),
        None => Err(ApiError::NotFound("".into())),
    }
}

#[derive(Deserialize)]
struct UpdateModPayload {
    featured: bool,
}

#[put("/v1/mods/{id}")]
pub async fn update_mod(
    data: web::Data<AppData>,
    path: web::Path<String>,
    payload: web::Json<UpdateModPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.into_developer()?;
    if !dev.admin {
        return Err(ApiError::Forbidden);
    }
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::DbError))?;
    if let Err(e) = Mod::update_mod(&path, payload.featured, &mut transaction).await {
        let _ = transaction.rollback().await;
        return Err(e);
    }
    if (transaction.commit().await).is_err() {
        return Err(ApiError::DbError);
    }

    Ok(HttpResponse::NoContent())
}
