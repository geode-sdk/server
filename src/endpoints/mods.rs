use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Acquire;

use crate::extractors::auth::Auth;
use crate::types::api::{create_download_link, ApiError, ApiResponse};
use crate::types::mod_json::ModJson;
use crate::types::models::developer::Developer;
use crate::types::models::mod_entity::{download_geode_file, Mod, ModUpdate};
use crate::types::models::mod_gd_version::{GDVersionEnum, VerPlatform};
use crate::AppData;

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndexSortType {
    #[default]
    Downloads,
    RecentlyUpdated,
    RecentlyPublished,
    Name,
    NameReverse,
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
    pub featured: Option<bool>,
    pub pending_validation: Option<bool>,
}

#[derive(Deserialize)]
pub struct CreateQueryParams {
    download_link: String,
}

#[get("/v1/mods")]
pub async fn index(
    data: web::Data<AppData>,
    query: web::Query<IndexQueryParams>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    if query.pending_validation.is_some() {
        let dev = auth.developer()?;
        if !dev.admin {
            return Err(ApiError::Forbidden);
        }
    }

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
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    // honestly might be worth putting this into a separate function
    // but i don't actually know where it'd go. yay
    let allow_invalid = if let Ok(dev) = auth.developer() {
        dev.admin ||
            Developer::has_access_to_mod(dev.id, &id, &mut pool)
                .await.unwrap_or(false)
    } else {
        false
    };

    let found = Mod::get_one(&id, allow_invalid, &mut pool).await?;
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
    let dev = auth.developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut file_path = download_geode_file(&payload.download_link).await?;
    let json = ModJson::from_zip(&mut file_path, &payload.download_link)?;
    json.validate()?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    let result = Mod::from_json(&json, dev, &mut transaction).await;
    if result.is_err() {
        transaction
            .rollback()
            .await
            .or(Err(ApiError::TransactionError))?;
        return Err(result.err().unwrap());
    }
    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;
    Ok(HttpResponse::NoContent())
}

#[derive(Deserialize)]
struct UpdateQueryParams {
    ids: String,
}
#[get("/v1/mods/updates")]
pub async fn get_mod_updates(
    data: web::Data<AppData>,
    query: web::Query<UpdateQueryParams>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let ids = query
        .ids
        .split(';')
        .map(String::from)
        .collect::<Vec<String>>();

    let platforms: Vec<VerPlatform> = vec![];

    let mut result: Vec<ModUpdate> = Mod::get_updates(ids, platforms, &mut pool).await?;
    for i in &mut result {
        i.download_link = create_download_link(&data.app_url, &i.id, &i.version);
    }

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: result,
    }))
}

#[get("/v1/mods/{id}/logo")]
pub async fn get_logo(
    data: web::Data<AppData>,
    path: web::Path<String>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let allow_invalid = if let Ok(dev) = auth.developer() {
        dev.admin ||
            Developer::has_access_to_mod(dev.id, &path, &mut pool)
                .await.unwrap_or(false)
    } else {
        false
    };

    let image = Mod::get_logo_for_mod(&path, allow_invalid, &mut pool).await?;

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
    let dev = auth.developer()?;
    if !dev.admin {
        return Err(ApiError::Forbidden);
    }
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if let Err(e) = Mod::update_mod(&path, payload.featured, &mut transaction).await {
        transaction
            .rollback()
            .await
            .or(Err(ApiError::TransactionError))?;
        return Err(e);
    }
    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    Ok(HttpResponse::NoContent())
}
