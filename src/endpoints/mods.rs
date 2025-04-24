use crate::config::AppData;
use crate::database::repository::dependencies;
use crate::database::repository::developers;
use crate::database::repository::incompatibilities;
use crate::database::repository::mod_gd_versions;
use crate::database::repository::mod_links;
use crate::database::repository::mod_tags;
use crate::database::repository::mod_versions;
use crate::database::repository::mods;
use crate::events::mod_feature::ModFeaturedEvent;
use crate::extractors::auth::Auth;
use crate::mod_zip;
use crate::types::api::{create_download_link, ApiError, ApiResponse};
use crate::types::mod_json::ModJson;
use crate::types::models::incompatibility::Incompatibility;
use crate::types::models::mod_entity::{Mod, ModUpdate};
use crate::types::models::mod_gd_version::{GDVersionEnum, VerPlatform};
use crate::types::models::mod_version::ModVersion;
use crate::types::models::mod_version_status::ModVersionStatusEnum;
use crate::webhook::discord::DiscordWebhook;
use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Acquire;

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
    pub geode: Option<String>,
    pub developer: Option<String>,
    pub tags: Option<String>,
    pub featured: Option<bool>,
    pub status: Option<ModVersionStatusEnum>,
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
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    if let Some(s) = query.status {
        if s == ModVersionStatusEnum::Rejected {
            let dev = auth.developer()?;
            if !dev.admin {
                return Err(ApiError::Forbidden);
            }
        }
    }

    let mut result = Mod::get_index(&mut pool, query.0).await?;
    for i in &mut result.data {
        for j in &mut i.versions {
            j.modify_metadata(data.app_url(), false);
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
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let has_extended_permissions = match auth.developer() {
        Ok(dev) => dev.admin || developers::has_access_to_mod(dev.id, &id, &mut pool).await?,
        _ => false,
    };

    let mut the_mod: Mod = mods::get_one(&id, true, &mut pool)
        .await?
        .ok_or(ApiError::NotFound(format!("Mod '{id}' not found")))?;

    the_mod.tags = mod_tags::get_for_mod(&the_mod.id, &mut pool)
        .await?
        .into_iter()
        .map(|t| t.name)
        .collect();
    the_mod.developers = developers::get_all_for_mod(&the_mod.id, &mut pool).await?;
    the_mod.versions = mod_versions::get_for_mod(
        &the_mod.id,
        Some(&[ModVersionStatusEnum::Accepted]),
        &mut pool,
    )
    .await?;
    for i in &mut the_mod.versions {
        i.modify_metadata(data.app_url(), has_extended_permissions);
    }

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: the_mod,
    }))
}

#[post("/v1/mods")]
pub async fn create(
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
    let bytes = mod_zip::download_mod(&payload.download_link, data.max_download_mb()).await?;
    let json = ModJson::from_zip(bytes, &payload.download_link, true)?;
    json.validate()?;

    let existing: Option<Mod> = mods::get_one(&json.id, false, &mut pool).await?;
    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let (mut the_mod, mut version) = if existing.is_none() {
        // Easy path: mod doesn't exist

        let mut created: Mod = mods::create(&json, &mut tx).await?;
        mods::assign_owner(&created.id, dev.id, &mut tx).await?;
        if let Some(tags) = &json.tags {
            let tag_list = mod_tags::parse_tag_list(tags, &mut tx).await?;
            mod_tags::update_for_mod(&created.id, &tag_list, &mut tx).await?;
        }
        if let Some(l) = json.links.clone() {
            created.links = Some(
                mod_links::upsert(&created.id, l.community, l.homepage, l.source, &mut tx).await?,
            );
        }

        // First version is always not accepted, even if the developer is verified
        let version = mod_versions::create_from_json(&json, false, &mut tx).await?;

        (created, version)
    } else {
        // Hard path: mod already exists

        let mut existing = existing.unwrap();

        if !developers::has_access_to_mod(dev.id, &existing.id, &mut tx).await? {
            return Err(ApiError::Forbidden);
        }

        existing.versions = mod_versions::get_for_mod(&existing.id, None, &mut tx).await?;

        let mut pending: Option<ModVersion> = None;
        let mut accepted: u32 = 0;
        let mut unlisted: u32 = 0;

        for i in &existing.versions {
            match i.status {
                ModVersionStatusEnum::Accepted => accepted += 1,
                ModVersionStatusEnum::Pending => pending = Some(i.clone()),
                ModVersionStatusEnum::Unlisted => unlisted += 1,
                _ => {}
            }
        }

        let has_accepted_versions = accepted > 0 || unlisted > 0;

        // Allow verified auto-accepted on updates, never on the first version
        let make_accepted = dev.verified && has_accepted_versions;

        let version = if !has_accepted_versions && pending.is_none() {
            mod_versions::create_from_json(&json, make_accepted, &mut tx).await?
        } else if has_accepted_versions && pending.is_none() {
            mod_versions::create_from_json(&json, dev.verified, &mut tx).await?
        } else {
            let pending_ver = pending.unwrap();

            dependencies::clear(pending_ver.id, &mut tx).await?;
            incompatibilities::clear(pending_ver.id, &mut tx).await?;
            mod_gd_versions::clear(pending_ver.id, &mut tx).await?;
            mod_versions::update_pending_version(pending_ver.id, &json, make_accepted, &mut tx)
                .await?
        };

        // If the new version gets accepted automatically, update stuff on the mod itself
        if make_accepted {
            // This copies 2 possibly very big strings. Cry about it >:)
            let mut existing = mods::update_with_json(existing, &json, &mut tx).await?;
            if let Some(tags) = &json.tags {
                let tag_list = mod_tags::parse_tag_list(tags, &mut tx).await?;
                mod_tags::update_for_mod(&existing.id, &tag_list, &mut tx).await?;
            }
            if let Some(l) = json.links.clone() {
                existing.links = Some(
                    mod_links::upsert(&existing.id, l.community, l.homepage, l.source, &mut tx)
                        .await?,
                );
            }

            (existing, version)
        } else {
            (existing, version)
        }
    };

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
    version.gd = mod_gd_versions::create(version.id, &json, &mut tx).await?;
    the_mod.developers = developers::get_all_for_mod(&the_mod.id, &mut tx).await?;

    // Remove the version (if we need an update on a pending version) and insert our new version
    the_mod.versions.retain(|x| x.id != version.id);
    the_mod.versions.insert(0, version);

    tx.commit().await.or(Err(ApiError::TransactionError))?;
    Ok(HttpResponse::Created().json(ApiResponse {
        error: "".into(),
        payload: the_mod,
    }))
}

#[derive(Deserialize)]
struct UpdateQueryParams {
    ids: String,
    gd: GDVersionEnum,
    platform: VerPlatform,
    geode: String,
}
#[get("/v1/mods/updates")]
pub async fn get_mod_updates(
    data: web::Data<AppData>,
    query: web::Query<UpdateQueryParams>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    if query.platform == VerPlatform::Android || query.platform == VerPlatform::Mac {
        return Err(ApiError::BadRequest("Invalid platform. Use android32 / android64 for android and mac-intel / mac-arm for mac".to_string()));
    }

    let ids = query
        .ids
        .split(';')
        .map(String::from)
        .collect::<Vec<String>>();

    let geode = match semver::Version::parse(&query.geode) {
        Ok(g) => g,
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::BadRequest(
                "Invalid geode version format".to_string(),
            ));
        }
    };

    let mut result: Vec<ModUpdate> =
        Mod::get_updates(&ids, query.platform, &geode, query.gd, &mut pool).await?;
    let mut replacements =
        Incompatibility::get_supersedes_for(&ids, query.platform, query.gd, &geode, &mut pool)
            .await?;

    for i in &mut result {
        if let Some(replacement) = replacements.get(&i.id) {
            let mut clone = replacement.clone();
            clone.download_link = create_download_link(data.app_url(), &clone.id, &clone.version);
            i.replacement = Some(clone);
            replacements.remove_entry(&i.id);
        }
        i.download_link = create_download_link(data.app_url(), &i.id, &i.version);
    }

    for i in replacements {
        let mut replacement = i.1.clone();
        replacement.download_link =
            create_download_link(data.app_url(), &replacement.id, &replacement.version);
        result.push(ModUpdate {
            id: i.0.clone(),
            version: "1.0.0".to_string(),
            mod_version_id: 0,
            download_link: replacement.download_link.clone(),
            replacement: Some(replacement),
            dependencies: vec![],
            incompatibilities: vec![],
        });
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
) -> Result<impl Responder, ApiError> {
    use crate::database::repository::*;
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    let image: Option<Vec<u8>> = mods::get_logo(&path.into_inner(), &mut pool).await?;

    match image {
        Some(i) => {
            if i.is_empty() {
                Ok(HttpResponse::NotFound().body(""))
            } else {
                Ok(HttpResponse::Ok().content_type("image/png").body(i))
            }
        }
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
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    let id = path.into_inner();
    let featured = mods::is_featured(&id, &mut pool).await?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if let Err(e) = Mod::update_mod(&id, payload.featured, &mut transaction).await {
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

    if featured != payload.featured {
        let item = Mod::get_one(&id, true, &mut pool).await?;
        if let Some(item) = item {
            let owner = developers::get_owner_for_mod(&id, &mut pool).await?;
            let first_ver = item.versions.first();
            if let Some(ver) = first_ver {
                ModFeaturedEvent {
                    id: item.id,
                    name: ver.name.clone(),
                    owner,
                    admin: dev,
                    base_url: data.app_url().to_string(),
                    featured: payload.featured,
                }
                .to_discord_webhook()
                .send(data.webhook_url());
            }
        }
    }

    Ok(HttpResponse::NoContent())
}
