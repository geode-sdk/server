use std::str::FromStr;

use actix_web::{dev::ConnectionInfo, get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::{types::ipnetwork::IpNetwork, Acquire};

use crate::{
    extractors::auth::Auth,
    types::{
        api::{ApiError, ApiResponse},
        mod_json::ModJson,
        models::{
            developer::Developer,
            download,
            mod_entity::{download_geode_file, Mod},
            mod_gd_version::{GDVersionEnum, VerPlatform},
            mod_version::ModVersion,
            mod_version_status::ModVersionStatusEnum,
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
    download_link: String,
}

#[derive(Deserialize)]
struct UpdatePayload {
    status: ModVersionStatusEnum,
    info: Option<String>,
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

#[derive(Deserialize)]
struct GetOneQuery {
    platforms: Option<String>,
    gd: Option<String>,
}

#[get("v1/mods/{id}/versions/{version}")]
pub async fn get_one(
    path: web::Path<GetOnePath>,
    data: web::Data<AppData>,
    query: web::Query<GetOneQuery>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let mut version = {
        if path.version == "latest" {
            let gd: Option<GDVersionEnum> = match query.gd {
                Some(ref gd) => Some(
                    GDVersionEnum::from_str(gd)
                        .or(Err(ApiError::BadRequest("Invalid gd".to_string())))?,
                ),
                None => None,
            };

            let mut platforms: Vec<VerPlatform> = vec![];

            if let Some(p) = &query.platforms {
                for x in p.split(',') {
                    match VerPlatform::from_str(x) {
                        Ok(v) => {
                            if v == VerPlatform::Android {
                                platforms.push(VerPlatform::Android32);
                                platforms.push(VerPlatform::Android64);
                            } else {
                                platforms.push(v);
                            }
                        }
                        Err(_) => {
                            return Err(ApiError::BadRequest("Invalid platform".to_string()));
                        }
                    }
                }
            };

            ModVersion::get_latest_for_mod(&path.id, gd, platforms, &mut pool).await?
        } else {
            ModVersion::get_one(&path.id, &path.version, &mut pool).await?
        }
    };

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
    info: ConnectionInfo,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mod_version = ModVersion::get_one(&path.id, &path.version, &mut pool).await?;
    let url = ModVersion::get_download_url(&path.id, &path.version, &mut pool).await?;

    let ip = match info.realip_remote_addr() {
        None => return Err(ApiError::InternalError),
        Some(i) => i,
    };
    let net: IpNetwork = ip.parse().or(Err(ApiError::InternalError))?;

    if download::create_download(net, mod_version.id, &mut pool).await? {
        ModVersion::calculate_cached_downloads(mod_version.id, &mut pool).await?;
        Mod::calculate_cached_downloads(&mod_version.mod_id, &mut pool).await?;
    }

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
    let dev = auth.developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    if Mod::get_one(&path.id, &mut pool).await?.is_none() {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    if !(Developer::has_access_to_mod(dev.id, &path.id, &mut pool).await?) {
        return Err(ApiError::Forbidden);
    }

    let mut file_path = download_geode_file(&payload.download_link).await?;
    let json = ModJson::from_zip(&mut file_path, &payload.download_link)
        .or(Err(ApiError::FilesystemError))?;
    if json.id != path.id {
        return Err(ApiError::BadRequest(format!(
            "Request id {} does not match mod.json id {}",
            path.id, json.id
        )));
    }
    json.validate()?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if let Err(e) = Mod::new_version(&json, dev, &mut transaction).await {
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

#[put("v1/mods/{id}/versions/{version}")]
pub async fn update_version(
    path: web::Path<UpdateVersionPath>,
    data: web::Data<AppData>,
    payload: web::Json<UpdatePayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    if !dev.admin {
        return Err(ApiError::Forbidden);
    }
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    let r = ModVersion::update_version(
        &path.id,
        &path.version,
        payload.status,
        payload.info.clone(),
        &mut transaction,
    )
    .await;
    if r.is_err() {
        transaction
            .rollback()
            .await
            .or(Err(ApiError::TransactionError))?;
        return Err(r.err().unwrap());
    }
    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    Ok(HttpResponse::NoContent())
}
