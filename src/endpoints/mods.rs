use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Acquire;

use crate::extractors::auth::Auth;
use crate::types::api::{create_download_link, ApiError, ApiResponse};
use crate::types::mod_json::ModJson;
use crate::types::models::developer::Developer;
use crate::types::models::incompatibility::Incompatibility;
use crate::types::models::mod_entity::{download_geode_file, Mod, ModUpdate};
use crate::types::models::mod_gd_version::{GDVersionEnum, VerPlatform};
use crate::types::models::mod_version_status::ModVersionStatusEnum;
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

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
            j.modify_metadata(&data.app_url, false);
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

    let has_extended_permissions = match auth.developer() {
        Ok(dev) => dev.admin || Developer::has_access_to_mod(dev.id, &id, &mut pool).await?,
        _ => false,
    };

    let found = Mod::get_one(&id, false, &mut pool).await?;
    match found {
        Some(mut m) => {
            for i in &mut m.versions {
                i.modify_metadata(&data.app_url, has_extended_permissions);
            }
            Ok(web::Json(ApiResponse {
                error: "".into(),
                payload: m,
            }))
        }
        None => Err(ApiError::NotFound(format!("Mod '{id}' not found"))),
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
    let mut file_path = download_geode_file(&payload.download_link, data.max_download_mb).await?;
    let json = ModJson::from_zip(
        &mut file_path,
        &payload.download_link,
        false,
        data.max_download_mb,
    )?;
    json.validate()?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    let result = Mod::from_json(&json, dev.clone(), &mut transaction).await;
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
    gd: GDVersionEnum,
    platform: VerPlatform,
    geode: String,
}
#[get("/v1/mods/updates")]
pub async fn get_mod_updates(
    data: web::Data<AppData>,
    query: web::Query<UpdateQueryParams>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

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
            clone.download_link = create_download_link(&data.app_url, &clone.id, &clone.version);
            i.replacement = Some(clone);
            replacements.remove_entry(&i.id);
        }
        i.download_link = create_download_link(&data.app_url, &i.id, &i.version);
    }

    for i in replacements {
        let mut replacement = i.1.clone();
        replacement.download_link =
            create_download_link(&data.app_url, &replacement.id, &replacement.version);
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let image = Mod::get_logo_for_mod(&path, &mut pool).await?;
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
