use std::str::FromStr;

use actix_web::{dev::ConnectionInfo, get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use sqlx::{types::ipnetwork::IpNetwork, Acquire};

use crate::{
    extractors::auth::Auth,
    types::{
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
    },
    AppData,
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
        i.modify_download_link(&data.app_url);
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

            let platform_string = query.platforms.clone().unwrap_or_default();
            let platforms = VerPlatform::parse_query_string(&platform_string);

            ModVersion::get_latest_for_mod(&path.id, gd, platforms, query.major, &mut pool).await?
        } else {
            ModVersion::get_one(&path.id, &path.version, true, false, &mut pool).await?
        }
    };

    version.modify_download_link(&data.app_url);
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

    let ip = match info.realip_remote_addr() {
        None => return Err(ApiError::InternalError),
        Some(i) => i,
    };
    let net: IpNetwork = ip.parse().or(Err(ApiError::InternalError))?;

    if download::create_download(net, mod_version.id, &mut pool).await? {
        let name = mod_version.mod_id.clone();
        let version = mod_version.version.clone();
        tokio::spawn(async move {
            if let Err(e) = ModVersion::calculate_cached_downloads(mod_version.id, &mut pool).await
            {
                log::error!(
                    "Failed to calculate cached downloads for mod version {}. Error: {}",
                    version,
                    e
                );
            }
            if let Err(e) = Mod::calculate_cached_downloads(&mod_version.mod_id, &mut pool).await {
                log::error!(
                    "Failed to calculate cached downloads for mod {}. Error: {}",
                    name,
                    e
                );
            }
        });
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

    let fetched_mod = Mod::get_one(&path.id, true, &mut pool).await?;

    if fetched_mod.is_none() {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    if !(Developer::has_access_to_mod(dev.id, &path.id, &mut pool).await?) {
        return Err(ApiError::Forbidden);
    }

    let mut file_path = download_geode_file(&payload.download_link).await?;
    let json = ModJson::from_zip(&mut file_path, &payload.download_link, dev.verified)
        .or(Err(ApiError::FilesystemError))?;
    if json.id != path.id {
        return Err(ApiError::BadRequest(format!(
            "Request id {} does not match mod.json id {}",
            path.id, json.id
        )));
    }
    let webhook = json!({
        "embeds": [
            {
                "title": format!(
                    "Mod updated! {} {} -> {}",
                    json.name, fetched_mod.unwrap().versions.last().unwrap().version, json.version
                ),
                "description": format!(
                    "https://geode-sdk.org/mods/{}\n\nOwned by: [{}](https://github.com/{})",
                    json.id, dev.display_name, dev.username
                ),
                "thumbnail": {
                    "url": format!("https://api.geode-sdk.org/v1/mods/{}/logo", json.id)
                }
            }
        ]
    });
 
    let _ = reqwest::Client::new()
        .post("https://ptb.discord.com/api/webhooks/1251962420264698006/8JPCXoKM16zOPERvmtItFZTf2VNGsOpl8xvuY-X_s4TyyTPHxxASftWBR4XjmrtBPgRr")
        .json(&webhook)
        .send()
        .await;

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
    let fetched_mod = Mod::get_one(path.id.as_str(), false, &mut pool).await?.unwrap(); // this is up here because the borrow checker gets fussy when it's below `transaction`
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
    let version = fetched_mod.versions.last().unwrap();
    let is_first_version = fetched_mod.versions.len() == 1;
    let owner = &fetched_mod.developers[0];
    let webhook = json!({
        "embeds": [
            {
                "title": if is_first_version { format!("New mod! {} {}", version.name, path.version) } else { format!("Mod updated! {} {} -> {}", version.name, fetched_mod.versions[
                    fetched_mod.versions.len() - 2
                ].version, version.version) },
                "description": format!(
                    "https://geode-sdk.org/mods/{}\n\nAccepted by: [{}](https://github.com/{})\nOwned by: [{}](https://github.com/{})",
                    fetched_mod.id, dev.display_name, dev.username, owner.display_name, owner.username
                ),
                "thumbnail": {
                    "url": format!("https://api.geode-sdk.org/v1/mods/{}/logo", fetched_mod.id)
                }
            }
        ]
    });
    
    let _ = reqwest::Client::new()
        .post("https://ptb.discord.com/api/webhooks/1251962420264698006/8JPCXoKM16zOPERvmtItFZTf2VNGsOpl8xvuY-X_s4TyyTPHxxASftWBR4XjmrtBPgRr")
        .json(&webhook)
        .send()
        .await;
    if let Err(e) = ModVersion::update_version(
        id,
        payload.status,
        payload.info.clone(),
        dev.id,
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


    Ok(HttpResponse::NoContent())
}
