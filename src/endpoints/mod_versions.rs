use std::str::FromStr;

use actix_web::{dev::ConnectionInfo, get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::{types::ipnetwork::IpNetwork, Acquire};

use crate::{
    extractors::auth::Auth, forum::create_or_update_thread, types::{
        api::{ApiError, ApiResponse},
        mod_json::{split_version_and_compare, ModJson},
        models::{
            developer::Developer,
            download,
            mod_entity::{download_geode_file, Mod},
            mod_gd_version::{GDVersionEnum, VerPlatform},
            mod_version::{self, ModVersion},
            mod_version_status::ModVersionStatusEnum,
        },
    }, webhook::send_webhook, AppData
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

    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let has_extended_permissions = match auth.developer() {
        Ok(dev) => dev.admin || Developer::has_access_to_mod(dev.id, &path.id, &mut pool).await?,
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
        i.modify_metadata(&data.app_url, has_extended_permissions);
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let has_extended_permissions = match auth.developer() {
        Ok(dev) => dev.admin || Developer::has_access_to_mod(dev.id, &path.id, &mut pool).await?,
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

    version.modify_metadata(&data.app_url, has_extended_permissions);
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
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

    if data.disable_downloads {
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

    if let Ok((downloaded_version, downloaded_mod)) =
        download::create_download(net, mod_version.id, &mod_version.mod_id, &mut pool).await
    {
        let name = mod_version.mod_id.clone();
        let version = mod_version.version.clone();

        // only accepted mods can have their download counts incremented
        // we'll just fix this once they're updated anyways

        if (downloaded_version || downloaded_mod)
            && mod_version.status == ModVersionStatusEnum::Accepted
        {
            tokio::spawn(async move {
                if downloaded_version {
                    // we must nest more
                    if let Err(e) = ModVersion::increment_downloads(mod_version.id, &mut pool).await
                    {
                        log::error!(
                            "Failed to increment downloads for mod version {}. Error: {}",
                            version,
                            e
                        );
                    }
                }

                if downloaded_mod {
                    if let Err(e) = Mod::increment_downloads(&mod_version.mod_id, &mut pool).await {
                        log::error!(
                            "Failed to increment downloads for mod {}. Error: {}",
                            name,
                            e
                        );
                    }
                }
            });
        }
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

    let fetched_mod = Mod::get_one(&path.id, false, &mut pool).await?;

    if fetched_mod.is_none() {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    if !(Developer::has_access_to_mod(dev.id, &path.id, &mut pool).await?) {
        return Err(ApiError::Forbidden);
    }

    // remove invalid characters from link - they break the location header on download
    let download_link: String = payload
        .download_link
        .chars()
        .filter(|c| c.is_ascii() && *c != '\0')
        .collect();

    let mut file_path = download_geode_file(&download_link, data.max_download_mb).await?;
    let json = ModJson::from_zip(
        &mut file_path,
        &download_link,
        dev.verified,
        data.max_download_mb,
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

    let accepted_count = ModVersion::get_accepted_count(&json.id, &mut transaction).await?;

    if dev.verified && accepted_count > 0 {
        send_webhook(
            json.id.clone(),
            json.name.clone(),
            json.version.clone(),
            true,
            Developer {
                id: dev.id,
                username: dev.username.clone(),
                display_name: dev.display_name.clone(),
                is_owner: true,
            },
            dev.clone(),
            data.webhook_url.clone(),
            data.app_url.clone(),
        )
        .await;
    }

    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    if !dev.verified || accepted_count == 0 {
        tokio::spawn(async move {
            if data.guild_id == 0 || data.channel_id == 0 || data.bot_token.is_empty() {
                log::error!("Discord configuration is not set up. Not creating forum threads.");
                return;
            }

            let m = fetched_mod.unwrap();
            let v_res = ModVersion::get_one(&path.id, &json.version, true, false, &mut pool).await;
            if v_res.is_err() {
                return;
            }
            let v = v_res.unwrap();
            create_or_update_thread(
                None,
                data.guild_id,
                data.channel_id,
                data.bot_token.clone(),
                m,
                v,
                None,
                data.app_url.clone(),
            ).await;
        });
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
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
        data.max_download_mb,
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

        let owner = Developer::fetch_for_mod(version.mod_id.as_str(), &mut pool)
            .await?
            .into_iter()
            .find(|dev| dev.is_owner);

        send_webhook(
            version.mod_id,
            version.name.clone(),
            version.version.clone(),
            is_update,
            owner.as_ref().unwrap().clone(),
            dev.clone(),
            data.webhook_url.clone(),
            data.app_url.clone(),
        )
        .await;
    }

    if payload.status == ModVersionStatusEnum::Accepted || payload.status == ModVersionStatusEnum::Rejected {
        tokio::spawn(async move {
            if data.guild_id == 0 || data.channel_id == 0 || data.bot_token.is_empty() {
                log::error!("Discord configuration is not set up. Not creating forum threads.");
                return;
            }

            let m_res = Mod::get_one(&path.id, false, &mut pool).await.ok().flatten();
            if m_res.is_none() {
                return;
            }
            let m = m_res.unwrap();
            let v_res = ModVersion::get_one(&path.id, &path.version, true, false, &mut pool).await;
            if v_res.is_err() {
                return;
            }
            let v = v_res.unwrap();
            create_or_update_thread(
                None,
                data.guild_id,
                data.channel_id,
                data.bot_token.clone(),
                m,
                v,
                Some(dev.clone()),
                data.app_url.clone(),
            ).await;
        });
    }

    Ok(HttpResponse::NoContent())
}
