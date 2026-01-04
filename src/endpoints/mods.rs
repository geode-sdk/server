use crate::config::AppData;
use crate::database::repository::dependencies;
use crate::database::repository::developers;
use crate::database::repository::incompatibilities;
use crate::database::repository::mod_gd_versions;
use crate::database::repository::mod_links;
use crate::database::repository::mod_tags;
use crate::database::repository::mod_unlist_history;
use crate::database::repository::mod_versions;
use crate::database::repository::mods;
use crate::endpoints::ApiError;
use crate::events::mod_feature::ModFeaturedEvent;
use crate::extractors::auth::Auth;
use crate::mod_zip;
use crate::types::api::{create_download_link, ApiResponse};
use crate::types::mod_json::ModJson;
use crate::types::models;
use crate::types::models::incompatibility::Incompatibility;
use crate::types::models::mod_entity::{Mod, ModUpdate};
use crate::types::models::mod_gd_version::{GDVersionEnum, VerPlatform};
use crate::types::models::mod_link::ModLinks;
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
    Oldest,
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
    let mut pool = data.db().acquire().await?;

    if let Some(s) = query.status {
        if s == ModVersionStatusEnum::Rejected {
            auth.check_admin()?;
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
    let dev = auth.developer().ok();
    let mut pool = data.db().acquire().await?;

    let has_extended_permissions = match auth.developer() {
        Ok(dev) => dev.admin || developers::has_access_to_mod(dev.id, &id, &mut pool).await?,
        _ => false,
    };

    let mut the_mod: Mod = mods::get_one(&id, true, &mut pool)
        .await?
        .ok_or(ApiError::NotFound(format!("Mod '{id}' not found")))?;

    let version_statuses = match dev {
        None => Some(vec![
            ModVersionStatusEnum::Accepted,
            ModVersionStatusEnum::Pending,
        ]),
        Some(d) => {
            if d.admin {
                None
            } else {
                Some(vec![
                    ModVersionStatusEnum::Accepted,
                    ModVersionStatusEnum::Pending,
                ])
            }
        }
    };

    the_mod.tags = mod_tags::get_for_mod(&the_mod.id, &mut pool)
        .await?
        .into_iter()
        .map(|t| t.name)
        .collect();
    the_mod.developers = developers::get_all_for_mod(&the_mod.id, &mut pool).await?;
    the_mod.versions =
        mod_versions::get_for_mod(&the_mod.id, version_statuses.as_deref(), &mut pool).await?;
    the_mod.links = ModLinks::fetch(&the_mod.id, &mut pool).await?;

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
    let mut pool = data.db().acquire().await?;
    let bytes = mod_zip::download_mod(&payload.download_link, data.max_download_mb()).await?;
    let json = ModJson::from_zip(bytes, &payload.download_link, false)?;
    json.validate()?;

    let existing: Option<Mod> = mods::get_one(&json.id, false, &mut pool).await?;

    if let Some(m) = &existing {
        if !developers::has_access_to_mod(dev.id, &m.id, &mut pool).await? {
            return Err(ApiError::Authorization);
        }

        let versions = mod_versions::get_for_mod(&m.id, None, &mut pool).await?;

        if !versions.is_empty() {
            return Ok(HttpResponse::Conflict().json(ApiResponse {
                error: format!("Mod {} already exists! Submit a new version.", m.id),
                payload: "",
            }));
        }
    }

    let mut tx = pool.begin().await?;

    let mod_already_exists = existing.is_some();

    // Wacky stuff
    let mut the_mod = if let Some(m) = existing {
        m
    } else {
        mods::create(&json, &mut tx).await?
    };

    if !mod_already_exists {
        mods::assign_owner(&the_mod.id, dev.id, &mut tx).await?;
    }

    if let Some(tags) = &json.tags {
        let tag_list = models::tag::parse_tag_list(tags, &the_mod.id, &mut tx).await?;
        mod_tags::update_for_mod(&the_mod.id, &tag_list, &mut tx).await?;
    }
    if let Some(l) = json.links.clone() {
        the_mod.links =
            Some(mod_links::upsert(&the_mod.id, l.community, l.homepage, l.source, &mut tx).await?);
    }

    // First version is always not accepted, even if the developer is verified
    let mut version = mod_versions::create_from_json(&json, false, &mut tx).await?;

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
    the_mod.versions.insert(0, version);

    tx.commit().await?;

    for i in &mut the_mod.versions {
        i.modify_metadata(data.app_url(), false);
    }

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
    let mut pool = data.db().acquire().await?;

    if query.platform == VerPlatform::Android || query.platform == VerPlatform::Mac {
        return Err(ApiError::BadRequest("Invalid platform. Use android32 / android64 for android and mac-intel / mac-arm for mac".to_string()));
    }

    let ids = query
        .ids
        .split(';')
        .map(String::from)
        .collect::<Vec<String>>();

    let geode = semver::Version::parse(&query.geode).map_err(|_| {
        ApiError::BadRequest(format!(
            "Invalid mod.json geode version semver: {}",
            query.geode
        ))
    })?;

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
    let mut pool = data.db().acquire().await?;
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
    featured: Option<bool>,
    unlisted: Option<bool>,
    details: Option<String>
}

#[put("/v1/mods/{id}")]
pub async fn update_mod(
    data: web::Data<AppData>,
    path: web::Path<String>,
    payload: web::Json<UpdateModPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    // Check admin only if featured flag is set
    if payload.featured.is_some() {
        auth.check_admin()?;
    }
    let mut pool = data.db().acquire().await?;
    let mut tx = pool.begin().await?;

    let id = path.into_inner();

    let the_mod = mods::get_one(&id, false, &mut tx).await?;

    if the_mod.is_none() {
        return Err(ApiError::NotFound("Mod not found".into()));
    }

    let the_mod = the_mod.unwrap();

    if let Some(unlisted) = payload.unlisted {
        // Check if we actually can do this
        if !dev.admin {
            if !developers::has_access_to_mod(dev.id, &the_mod.id, &mut tx).await? {
                return Err(ApiError::Authorization);
            }

            if let Some(last_update) =
                mod_unlist_history::get_last_for_mod(&the_mod.id, &mut tx).await?
            {
                let modified_by = developers::get_one(last_update.modified_by, &mut tx).await?;

                // If the mod was unlisted by an admin, it can only be relisted by an admin
                if modified_by.is_some_and(|x| x.admin && last_update.unlisted == true && unlisted == false) {
                    return Err(ApiError::Authorization);
                }
            }
        }
    }

    if payload.featured.is_none() && payload.unlisted.is_none() {
        return Err(ApiError::BadRequest("No fields sent".into()));
    }

    let previous_mod = the_mod.clone();
    let the_mod = mods::user_update(the_mod, payload.featured, payload.unlisted, &mut tx).await?;

    let response = Ok(HttpResponse::NoContent());

    // If unlisted was updated, add it to the audit table
    if the_mod.unlisted != previous_mod.unlisted {
        mod_unlist_history::create(
            &the_mod.id,
            the_mod.unlisted,
            payload.details.clone(),
            dev.id,
            &mut tx
        ).await?;
    }

    tx.commit().await?;

    if let Some(featured) = payload.featured {
        if the_mod.featured == previous_mod.featured {
            return response;
        }

        let latest_version = mod_versions::get_latest_for_mod(
            &the_mod.id,
            Some(&[ModVersionStatusEnum::Accepted]),
            &mut pool,
        )
        .await?;
        if latest_version.is_none() {
            return response;
        }
        let latest_version = latest_version.unwrap();

        let owner = developers::get_owner_for_mod(&id, &mut pool).await?;
        if owner.is_none() {
            return response;
        }
        let owner = owner.unwrap();

        ModFeaturedEvent {
            id: the_mod.id,
            name: latest_version.name,
            owner,
            admin: dev,
            base_url: data.app_url().to_string(),
            featured,
        }
        .to_discord_webhook()
        .send(data.webhook_url());
    }

    response
}
