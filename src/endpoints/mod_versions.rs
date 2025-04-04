use std::str::FromStr;

use actix_web::{dev::ConnectionInfo, get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::{types::ipnetwork::IpNetwork, Acquire};

use crate::config::AppData;
use crate::database::repository::{developers, mod_downloads, mod_versions, mods};
use crate::events::mod_created::{
    NewModAcceptedEvent, NewModVersionAcceptedEvent, NewModVersionVerification,
};
use crate::webhook::discord::DiscordWebhook;
use crate::{
    extractors::auth::Auth,
    types::{
        api::{ApiError, ApiResponse},
        mod_json::{split_version_and_compare, ModJson},
        models::{
            mod_entity::{download_geode_file, Mod},
            mod_gd_version::{GDVersionEnum, VerPlatform},
            mod_version::{self, ModVersion},
            mod_version_status::ModVersionStatusEnum,
        },
    },
};

#[derive(Deserialize)]
struct IndexPath {
    id: String,
}

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
    major: Option<u32>,
}

#[derive(Deserialize)]
struct IndexQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    #[serde(default)]
    gd: Option<GDVersionEnum>,
    platforms: Option<String>,
    status: Option<ModVersionStatusEnum>,
    compare: Option<String>,
}

#[get("v1/mods/{id}/versions")]
pub async fn get_version_index(
    path: web::Path<IndexPath>,
    data: web::Data<AppData>,
    query: web::Query<IndexQuery>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let platforms = VerPlatform::parse_query_string(&query.platforms.clone().unwrap_or_default());
    let compare = query.compare.as_ref().map(|c| split_version_and_compare(c));

    if compare.is_some() && compare.as_ref().unwrap().is_err() {
        return Err(ApiError::BadRequest(format!(
            "Bad compare string {}",
            query.compare.as_ref().unwrap()
        )));
    }

    let compare = compare.map(|x| x.unwrap());

    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let has_extended_permissions = match auth.developer() {
        Ok(dev) => dev.admin || developers::has_access_to_mod(dev.id, &path.id, &mut pool).await?,
        _ => false,
    };

    let mut result = ModVersion::get_index(
        mod_version::IndexQuery {
            mod_id: path.id.clone(),
            page: query.page.unwrap_or(1),
            per_page: query.per_page.unwrap_or(10),
            compare,
            gd: query.gd,
            platforms,
            status: query.status.unwrap_or(ModVersionStatusEnum::Accepted),
        },
        &mut pool,
    )
    .await?;
    for i in &mut result.data {
        i.modify_metadata(data.app_url(), has_extended_permissions);
    }

    Ok(web::Json(ApiResponse {
        payload: result,
        error: "".to_string(),
    }))
}

#[get("v1/mods/{id}/versions/{version}")]
pub async fn get_one(
    path: web::Path<GetOnePath>,
    data: web::Data<AppData>,
    query: web::Query<GetOneQuery>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let has_extended_permissions = match auth.developer() {
        Ok(dev) => dev.admin || developers::has_access_to_mod(dev.id, &path.id, &mut pool).await?,
        _ => false,
    };

    let mut version = {
        if path.version == "latest" {
            let gd: Option<GDVersionEnum> = match query.gd {
                Some(ref gd) => Some(
                    GDVersionEnum::from_str(gd)
                        .or(Err(ApiError::BadRequest("Invalid gd".to_string())))?,
                ),
                None => None,
            };

            let platform_string = query.platforms.clone().unwrap_or_default();
            let platforms = VerPlatform::parse_query_string(&platform_string);

            ModVersion::get_latest_for_mod(&path.id, gd, platforms, query.major, &mut pool).await?
        } else {
            ModVersion::get_one(&path.id, &path.version, true, false, &mut pool).await?
        }
    };

    version.modify_metadata(data.app_url(), has_extended_permissions);
    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: version,
    }))
}

#[derive(Deserialize)]
struct DownloadQuery {
    gd: Option<GDVersionEnum>,
    // platform1,platform2,...
    platforms: Option<String>,
    major: Option<u32>,
}

#[get("v1/mods/{id}/versions/{version}/download")]
pub async fn download_version(
    path: web::Path<GetOnePath>,
    data: web::Data<AppData>,
    query: web::Query<DownloadQuery>,
    info: ConnectionInfo,
) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    let mod_version = {
        if path.version == "latest" {
            let platform_str = query.platforms.clone().unwrap_or_default();
            let platforms = VerPlatform::parse_query_string(&platform_str);
            ModVersion::get_latest_for_mod(&path.id, query.gd, platforms, query.major, &mut pool)
                .await?
        } else {
            ModVersion::get_one(&path.id, &path.version, false, false, &mut pool).await?
        }
    };
    let url = mod_version.download_link;

    if data.disable_downloads() || mod_version.status != ModVersionStatusEnum::Accepted {
        // whatever
        return Ok(HttpResponse::Found()
            .append_header(("Location", url))
            .finish());
    }

    let ip = match info.realip_remote_addr() {
        None => return Err(ApiError::InternalError),
        Some(i) => i,
    };
    let net: IpNetwork = ip.parse().or(Err(ApiError::InternalError))?;

    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let downloaded_mod_previously =
        mod_downloads::has_downloaded_mod(net, &mod_version.mod_id, &mut tx).await?;
    let inserted = mod_downloads::create(net, mod_version.id, &mut tx).await?;

    if inserted {
        mod_versions::increment_downloads(mod_version.id, &mut tx).await?;

        if !downloaded_mod_previously {
            mods::increment_downloads(&mod_version.mod_id, &mut tx).await?;
        }
    }

    let _ = tx.commit().await;

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
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let fetched_mod = Mod::get_one(&path.id, false, &mut pool).await?;

    if fetched_mod.is_none() {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    if !(developers::has_access_to_mod(dev.id, &path.id, &mut pool).await?) {
        return Err(ApiError::Forbidden);
    }

    // remove invalid characters from link - they break the location header on download
    let download_link: String = payload
        .download_link
        .chars()
        .filter(|c| c.is_ascii() && *c != '\0')
        .collect();

    let mut file_path = download_geode_file(&download_link, data.max_download_mb()).await?;
    let json = ModJson::from_zip(
        &mut file_path,
        &download_link,
        dev.verified,
        data.max_download_mb(),
    )
    .map_err(|err| {
        log::error!("Failed to parse mod.json: {}", err);
        ApiError::FilesystemError
    })?;
    if json.id != path.id {
        return Err(ApiError::BadRequest(format!(
            "Request id {} does not match mod.json id {}",
            path.id, json.id
        )));
    }

    json.validate()?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if let Err(e) = Mod::new_version(&json, &dev, &mut transaction).await {
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

    let approved_count = ModVersion::get_accepted_count(&json.id, &mut pool).await?;

    if dev.verified && approved_count != 0 {
        let owner = developers::get_owner_for_mod(&json.id, &mut pool).await?;

        NewModVersionAcceptedEvent {
            id: json.id.clone(),
            name: json.name.clone(),
            version: json.version.clone(),
            owner,
            verified: NewModVersionVerification::VerifiedDev,
            base_url: data.app_url().to_string(),
        }
        .to_discord_webhook()
        .send(data.webhook_url());
    }
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
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    let version = ModVersion::get_one(
        path.id.as_str(),
        path.version.as_str(),
        false,
        false,
        &mut pool,
    )
    .await?;
    let approved_count = ModVersion::get_accepted_count(version.mod_id.as_str(), &mut pool).await?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    let id = match sqlx::query!(
        "select id from mod_versions where mod_id = $1 and version = $2",
        &path.id,
        path.version.trim_start_matches('v')
    )
    .fetch_optional(&mut *transaction)
    .await
    {
        Ok(Some(id)) => id.id,
        Ok(None) => {
            return Err(ApiError::NotFound(String::from("Not Found")));
        }
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
    };

    if let Err(e) = ModVersion::update_version(
        id,
        payload.status,
        payload.info.clone(),
        dev.id,
        data.max_download_mb(),
        &mut transaction,
    )
    .await
    {
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

    if payload.status == ModVersionStatusEnum::Accepted {
        let is_update = approved_count > 0;

        let owner = developers::get_owner_for_mod(&version.mod_id, &mut pool).await?;

        if !is_update {
            NewModAcceptedEvent {
                id: version.mod_id,
                name: version.name.clone(),
                version: version.version.clone(),
                owner,
                verified_by: dev,
                base_url: data.app_url().to_string(),
            }
            .to_discord_webhook()
            .send(data.webhook_url());
        } else {
            NewModVersionAcceptedEvent {
                id: version.mod_id,
                name: version.name.clone(),
                version: version.version.clone(),
                owner,
                verified: NewModVersionVerification::Admin(dev),
                base_url: data.app_url().to_string(),
            }
            .to_discord_webhook()
            .send(data.webhook_url());
        }
    }

    Ok(HttpResponse::NoContent())
}
