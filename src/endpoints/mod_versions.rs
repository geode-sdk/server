use std::str::FromStr;

use actix_web::{dev::ConnectionInfo, get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::{types::ipnetwork::IpNetwork, Acquire};

use crate::config::AppData;
use crate::database::repository::{
    dependencies, developers, incompatibilities, mod_downloads, mod_gd_versions, mod_links,
    mod_tags, mod_versions, mods,
};
use crate::endpoints::ApiError;
use crate::events::mod_created::{
    NewModAcceptedEvent, NewModVersionAcceptedEvent, NewModVersionVerification,
};
use crate::mod_zip::{self, download_mod};
use crate::types::models;
use crate::webhook::discord::DiscordWebhook;
use crate::{
    extractors::auth::Auth,
    types::{
        api::ApiResponse,
        mod_json::{split_version_and_compare, ModJson},
        models::{
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
    let platforms = VerPlatform::parse_query_string(&query.platforms.clone().unwrap_or_default())?;
    let compare = query.compare.as_ref().map(|c| split_version_and_compare(c));

    if compare.is_some() && compare.as_ref().unwrap().is_err() {
        return Err(ApiError::BadRequest(format!(
            "Bad compare string {}",
            query.compare.as_ref().unwrap()
        )));
    }

    let compare = compare.map(|x| x.unwrap());

    let mut pool = data.db().acquire().await?;

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
    let mut pool = data.db().acquire().await?;

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
            let platforms = VerPlatform::parse_query_string(&platform_string)?;

            ModVersion::get_latest_for_mod(&path.id, gd, platforms, query.major, &mut pool).await?
        } else {
            ModVersion::get_one(&path.id, &path.version, true, false, &mut pool).await?
        }
    }
    .ok_or(ApiError::NotFound(
        "Couldn't find valid mod version for given filters".into(),
    ))?;

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
    let mut pool = data.db().acquire().await?;
    let mod_version = {
        if path.version == "latest" {
            let platform_str = query.platforms.clone().unwrap_or_default();
            let platforms = VerPlatform::parse_query_string(&platform_str)?;
            ModVersion::get_latest_for_mod(&path.id, query.gd, platforms, query.major, &mut pool)
                .await?
        } else {
            ModVersion::get_one(&path.id, &path.version, false, false, &mut pool).await?
        }
    }
    .ok_or(ApiError::NotFound(
        "Couldn't find valid mod version for given filters".into(),
    ))?;
    let url = mod_version.download_link;

    if data.disable_downloads() || mod_version.status != ModVersionStatusEnum::Accepted {
        // whatever
        return Ok(HttpResponse::Found()
            .append_header(("Location", url))
            .finish());
    }

    let Some(ip) = info.realip_remote_addr() else {
        return Err(ApiError::InternalError(
            "Couldn't detect IP address of client".into(),
        ));
    };

    let net: IpNetwork = ip.parse().or(Err(ApiError::InternalError))?;

    let mut tx = pool.begin().await?;

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
    path: web::Path<String>,
    data: web::Data<AppData>,
    payload: web::Json<CreateQueryParams>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;

    let id = path.into_inner();

    let the_mod = mods::get_one(&id, false, &mut pool)
        .await?
        .ok_or(ApiError::NotFound(format!("Mod {} not found", &id)))?;

    if !(developers::has_access_to_mod(dev.id, &the_mod.id, &mut pool).await?) {
        return Err(ApiError::Authorization);
    }

    let versions = mod_versions::get_for_mod(
        &the_mod.id,
        Some(&[
            ModVersionStatusEnum::Accepted,
            ModVersionStatusEnum::Pending,
            ModVersionStatusEnum::Unlisted,
        ]),
        &mut pool,
    )
    .await?;

    let accepted_versions = versions
        .iter()
        .filter(|i| {
            i.status == ModVersionStatusEnum::Accepted || i.status == ModVersionStatusEnum::Unlisted
        })
        .count();

    let make_accepted = match accepted_versions {
        0 => false,
        _ => dev.verified,
    };

    // remove invalid characters from link - they break the location header on download
    let download_link: String = payload
        .download_link
        .chars()
        .filter(|c| c.is_ascii() && *c != '\0')
        .collect();

    let bytes = download_mod(&download_link, data.max_download_mb()).await?;
    let json = ModJson::from_zip(bytes, &download_link, make_accepted)
        .inspect_err(|e| log::error!("Failed to parse mod.json: {e}"))?;
    if json.id != the_mod.id {
        return Err(ApiError::BadRequest(format!(
            "Request id {} does not match mod.json id {}",
            the_mod.id, json.id
        )));
    }

    json.validate()?;

    let mut tx = pool.begin().await?;

    let mut version: ModVersion = if versions.is_empty() {
        mod_versions::create_from_json(&json, make_accepted, &mut tx).await?
    } else {
        let latest = versions.first().unwrap();
        let latest_version = semver::Version::parse(&latest.version)
            .inspect_err(|e| log::error!("Failed to parse locally stored version: {}", e))
            .or(Err(ApiError::InternalError))?;
        let new_version = semver::Version::parse(json.version.trim_start_matches('v')).or(Err(
            ApiError::BadRequest(format!("Invalid mod.json version: {}", json.version)),
        ))?;

        if new_version == latest_version {
            return Err(ApiError::BadRequest(format!(
                "mod.json has the same version as the latest version: {}",
                new_version
            )));
        }

        if new_version < latest_version {
            return Err(ApiError::BadRequest(format!(
                "mod.json version {} is less than latest mod version {}",
                json.version, latest_version
            )));
        }

        if latest.status == ModVersionStatusEnum::Pending {
            // clear everything and update the version
            dependencies::clear(latest.id, &mut tx).await?;
            incompatibilities::clear(latest.id, &mut tx).await?;
            mod_gd_versions::clear(latest.id, &mut tx).await?;
            mod_versions::update_pending_version(latest.id, &json, make_accepted, &mut tx).await?
        } else {
            mod_versions::create_from_json(&json, make_accepted, &mut tx).await?
        }
    };

    version.gd = mod_gd_versions::create(version.id, &json, &mut tx).await?;
    version.dependencies = Some(
        dependencies::create(version.id, &json, &mut tx)
            .await?
            .into_iter()
            .map(|x| x.into_response())
            .collect(),
    );
    version.incompatibilities = Some(
        incompatibilities::create(version.id, &json, &mut tx)
            .await?
            .into_iter()
            .map(|x| x.into_response())
            .collect(),
    );

    if make_accepted {
        if let Some(links) = json.links.clone() {
            mod_links::upsert(
                &the_mod.id,
                links.community,
                links.homepage,
                links.source,
                &mut tx,
            )
            .await?;
        }
        if let Some(tags) = &json.tags {
            if !tags.is_empty() {
                let tags = models::tag::parse_tag_list(tags, &mut tx).await?;
                mod_tags::update_for_mod(&the_mod.id, &tags, &mut tx).await?;
            }
        }

        mods::update_with_json_moved(the_mod, json, &mut tx).await?;
    }

    tx.commit().await?;

    if make_accepted {
        let owner = developers::get_owner_for_mod(&version.mod_id, &mut pool)
            .await?
            .ok_or(ApiError::BadRequest("Mod doesn't have an owner".into()))?;

        NewModVersionAcceptedEvent {
            id: version.mod_id.clone(),
            name: version.name.clone(),
            version: version.version.clone(),
            owner,
            verified: NewModVersionVerification::VerifiedDev,
            base_url: data.app_url().to_string(),
        }
        .to_discord_webhook()
        .send(data.webhook_url());
    }

    version.modify_metadata(data.app_url(), false);

    Ok(HttpResponse::Created().json(ApiResponse {
        error: "".into(),
        payload: version,
    }))
}

#[put("v1/mods/{id}/versions/{version}")]
pub async fn update_version(
    path: web::Path<UpdateVersionPath>,
    data: web::Data<AppData>,
    payload: web::Json<UpdatePayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;

    let mut pool = data.db().acquire().await?;

    let the_mod = mods::get_one(&path.id, false, &mut pool)
        .await?
        .ok_or(ApiError::NotFound(format!("Mod {} not found", path.id)))?;

    if !dev.admin {
        return Err(ApiError::Authorization);
    }

    let version = mod_versions::get_by_version_str(&the_mod.id, &path.version, &mut pool)
        .await?
        .ok_or(ApiError::NotFound(format!(
            "Version {} not found",
            path.version
        )))?;

    if version.status == payload.status {
        return Ok(HttpResponse::NoContent());
    }

    if payload.status == ModVersionStatusEnum::Pending {
        return Err(ApiError::BadRequest(
            "Cannot change version status to pending".into(),
        ));
    }

    let approved_count = ModVersion::get_accepted_count(version.mod_id.as_str(), &mut pool).await?;
    let mut tx = pool.begin().await?;

    let old_status = version.status;
    let version = mod_versions::update_version_status(
        version,
        payload.status,
        payload.info.as_deref(),
        &dev,
        &mut tx,
    )
    .await?;

    if old_status == ModVersionStatusEnum::Pending
        && version.status == ModVersionStatusEnum::Accepted
    {
        if approved_count == 0 {
            // Used to push new mods to the top of the "Recently created" list
            mods::touch_created_at(&the_mod.id, &mut tx).await?;
        }

        let bytes = mod_zip::download_mod_hash_comp(
            &version.download_link,
            &version.hash,
            data.max_download_mb(),
        )
        .await?;

        let json = ModJson::from_zip(bytes, &version.download_link, true)?;

        // Update links with data from mod.json
        if let Some(links) = json.links.clone() {
            mod_links::upsert(
                the_mod.id.as_str(),
                links.community,
                links.homepage,
                links.source,
                &mut tx,
            )
            .await?;
        } else {
            mod_links::upsert(the_mod.id.as_str(), None, None, None, &mut tx).await?;
        }

        // Update tags with data from mod.json
        let tags = if let Some(tags) = &json.tags {
            models::tag::parse_tag_list(tags, &mut tx).await?
        } else {
            vec![]
        };

        mod_tags::update_for_mod(&the_mod.id, &tags, &mut tx).await?;

        mods::update_with_json_moved(the_mod, json, &mut tx).await?;
    }

    tx.commit().await?;

    if payload.status == ModVersionStatusEnum::Accepted {
        let is_update = approved_count > 0;

        let owner = developers::get_owner_for_mod(&version.mod_id, &mut pool)
            .await?
            .ok_or(Err(ApiError::InternalError(
                "Couldn't find owner for mod".into(),
            )))?;

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
