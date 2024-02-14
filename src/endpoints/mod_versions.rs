use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Acquire;

use crate::{
    extractors::auth::Auth,
    types::{
        api::{ApiError, ApiResponse},
        mod_json::ModJson,
        models::{
            developer::Developer,
            mod_entity::{download_geode_file, Mod},
            mod_version::ModVersion,
        },
    },
    AppData,
};

#[derive(Deserialize)]
pub struct GetOnePath {
    id: String,
    version: String,
}

#[derive(Deserialize)]
pub struct CreateQueryParams {
    download_url: String,
}

#[derive(Deserialize)]
struct UpdatePayload {
    validated: Option<bool>,
    unlisted: Option<bool>,
}

#[derive(Deserialize)]
pub struct CreateVersionPath {
    id: String,
}

#[derive(Deserialize)]
struct UpdateVersionPath {
    id: String,
    version: String,
}

#[get("v1/mods/{id}/versions/{version}")]
pub async fn get_one(
    path: web::Path<GetOnePath>,
    data: web::Data<AppData>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut version = ModVersion::get_one(&path.id, &path.version, &mut pool).await?;
    version.modify_download_link(&data.app_url);
    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: version,
    }))
}

#[get("v1/mods/{id}/versions/{version}/download")]
pub async fn download_version(
    path: web::Path<GetOnePath>,
    data: web::Data<AppData>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let url = ModVersion::get_download_url(&path.id, &path.version, &mut pool).await?;
    Ok(HttpResponse::Found()
        .append_header(("Location", url))
        .finish())
}

#[post("v1/mods/{id}/versions")]
pub async fn create_version(
    path: web::Path<CreateVersionPath>,
    data: web::Data<AppData>,
    payload: web::Json<CreateQueryParams>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.into_developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    if Mod::get_one(&path.id, &mut pool).await?.is_none() {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    if !(Developer::has_access_to_mod(dev.id, &path.id, &mut pool).await?) {
        return Err(ApiError::Forbidden);
    }

    let mut file_path = download_geode_file(&payload.download_url).await?;
    let json = ModJson::from_zip(&mut file_path, payload.download_url.as_str())
        .or(Err(ApiError::FilesystemError))?;
    if json.id != path.id {
        return Err(ApiError::BadRequest(format!(
            "Request id {} does not match mod.json id {}",
            path.id, json.id
        )));
    }
    let mut transaction = pool.begin().await.or(Err(ApiError::DbError))?;
    if let Err(e) = Mod::new_version(&json, dev, &mut transaction).await {
        let _ = transaction.rollback().await;
        return Err(e);
    }
    let _ = transaction.commit().await;
    Ok(HttpResponse::NoContent())
}

#[put("v1/mods/{id}/versions/{version}")]
pub async fn update_version(
    path: web::Path<UpdateVersionPath>,
    data: web::Data<AppData>,
    payload: web::Json<UpdatePayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.into_developer()?;
    if !dev.admin {
        return Err(ApiError::Forbidden);
    }
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::DbError))?;
    let r = ModVersion::update_version(
        &path.id,
        &path.version,
        payload.validated,
        payload.unlisted,
        &mut transaction,
    )
    .await;
    if r.is_err() {
        transaction.rollback().await.or(Err(ApiError::DbError))?;
        return Err(r.err().unwrap());
    }
    let r = Mod::try_update_latest_version(&path.id, &mut transaction).await;
    if r.is_err() {
        transaction.rollback().await.or(Err(ApiError::DbError))?;
        return Err(r.err().unwrap());
    }
    transaction.commit().await.or(Err(ApiError::DbError))?;

    Ok(HttpResponse::NoContent())
}
