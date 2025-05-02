use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::config::AppData;
use crate::database::repository::{auth_tokens, developers, mods, refresh_tokens};
use crate::{
    extractors::auth::Auth,
    types::{
        api::{ApiError, ApiResponse},
        models::{
            developer::ModDeveloper, mod_entity::Mod, mod_version_status::ModVersionStatusEnum,
        },
    },
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SimpleDevMod {
    pub id: String,
    pub featured: bool,
    pub download_count: i32,
    pub versions: Vec<SimpleDevModVersion>,
    pub developers: Vec<ModDeveloper>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SimpleDevModVersion {
    pub name: String,
    pub version: String,
    pub download_count: i32,
    pub validated: bool,
    pub info: Option<String>,
    pub status: ModVersionStatusEnum,
}

#[derive(Deserialize)]
struct AddDevPath {
    id: String,
}

#[derive(Deserialize)]
struct RemoveDevPath {
    id: String,
    username: String,
}

#[derive(Deserialize)]
struct AddDevPayload {
    username: String,
}

#[derive(Deserialize)]
struct DeveloperUpdatePayload {
    admin: Option<bool>,
    verified: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateDeveloperPath {
    id: i32,
}

#[derive(Deserialize)]
struct DeveloperIndexQuery {
    query: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

#[get("v1/developers")]
pub async fn developer_index(
    data: web::Data<AppData>,
    query: web::Query<DeveloperIndexQuery>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let page: i64 = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

    let query = query.query.clone().unwrap_or_default();

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: developers::index(&query, page, per_page, &mut pool).await?,
    }))
}

#[post("v1/mods/{id}/developers")]
pub async fn add_developer_to_mod(
    data: web::Data<AppData>,
    path: web::Path<AddDevPath>,
    json: web::Json<AddDevPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }
    if !developers::owns_mod(dev.id, &path.id, &mut pool).await? {
        return Err(ApiError::Forbidden);
    }

    let target = developers::get_one_by_username(&json.username, &mut pool)
        .await?
        .ok_or(ApiError::BadRequest(format!(
            "No developer found with username {}",
            json.username
        )))?;

    mods::assign_developer(&path.id, target.id, false, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}

#[delete("v1/mods/{id}/developers/{username}")]
pub async fn remove_dev_from_mod(
    data: web::Data<AppData>,
    path: web::Path<RemoveDevPath>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }

    if !developers::owns_mod(dev.id, &path.id, &mut pool).await? {
        return Err(ApiError::Forbidden);
    }

    let target = developers::get_one_by_username(&path.username, &mut pool)
        .await?
        .ok_or(ApiError::NotFound(format!(
            "No developer found with username {}",
            path.username
        )))?;

    if target.id == dev.id {
        return Ok(HttpResponse::Conflict().json(ApiResponse {
            error: "Cannot remove self from mod developer list".into(),
            payload: "",
        }));
    }

    if !developers::has_access_to_mod(target.id, &path.id, &mut pool).await? {
        return Ok(HttpResponse::NotFound().json(ApiResponse {
            error: format!("{} is not a developer for this mod", target.username),
            payload: "",
        }));
    }

    mods::unassign_developer(&path.id, target.id, &mut pool).await?;

    Ok(HttpResponse::NoContent().finish())
}

#[delete("v1/me/token")]
pub async fn delete_token(
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let token = auth.token()?;
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    auth_tokens::remove_token(token, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}

#[delete("v1/me/tokens")]
pub async fn delete_tokens(
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    auth_tokens::remove_developer_tokens(dev.id, &mut pool).await?;
    refresh_tokens::remove_developer_tokens(dev.id, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}

#[derive(Deserialize)]
struct UploadProfilePayload {
    display_name: String,
}

#[put("v1/me")]
pub async fn update_profile(
    data: web::Data<AppData>,
    json: web::Json<UploadProfilePayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    if !json
        .display_name
        .chars()
        .all(|x| char::is_ascii_alphanumeric(&x))
    {
        return Err(ApiError::BadRequest(
            "Display name must contain only ASCII alphanumeric characters".into(),
        ));
    }

    if json.display_name.len() < 2 {
        return Err(ApiError::BadRequest(
            "Display name must have > 1 character".into(),
        ));
    }

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: developers::update_profile(dev.id, &json.display_name, &mut pool).await?,
    }))
}

#[derive(Deserialize)]
struct GetOwnModsQuery {
    #[serde(default = "default_own_mods_status")]
    status: ModVersionStatusEnum,
}

pub fn default_own_mods_status() -> ModVersionStatusEnum {
    ModVersionStatusEnum::Accepted
}

#[get("v1/me/mods")]
pub async fn get_own_mods(
    data: web::Data<AppData>,
    query: web::Query<GetOwnModsQuery>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    let mods: Vec<SimpleDevMod> = Mod::get_all_for_dev(dev.id, query.status, &mut pool).await?;
    Ok(HttpResponse::Ok().json(ApiResponse {
        error: "".to_string(),
        payload: mods,
    }))
}

#[get("v1/me")]
pub async fn get_me(auth: Auth) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    Ok(HttpResponse::Ok().json(ApiResponse {
        error: "".to_string(),
        payload: dev,
    }))
}

#[derive(Deserialize)]
struct GetDeveloperPath {
    id: i32,
}

#[get("v1/developers/{id}")]
pub async fn get_developer(
    data: web::Data<AppData>,
    path: web::Path<GetDeveloperPath>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    let result = developers::get_one(path.id, &mut pool)
        .await?
        .ok_or(ApiError::NotFound("Developer not found".into()))?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: result,
    }))
}

#[put("v1/developers/{id}")]
pub async fn update_developer(
    auth: Auth,
    data: web::Data<AppData>,
    path: web::Path<UpdateDeveloperPath>,
    payload: web::Json<DeveloperUpdatePayload>,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    auth.admin()?;

    if payload.admin.is_none() && payload.verified.is_none() {
        return Err(ApiError::BadRequest(
            "Specify at least one param to modify".into(),
        ));
    }

    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    if payload.admin.is_some() && dev.id == path.id {
        return Err(ApiError::BadRequest(
            "Can't override your own admin status".into(),
        ));
    }

    let updating = {
        if dev.id == path.id {
            dev
        } else {
            developers::get_one(path.id, &mut pool)
                .await?
                .ok_or(ApiError::NotFound("Developer not found".into()))?
        }
    };

    let result = developers::update_status(
        path.id,
        payload.verified.unwrap_or(updating.verified),
        payload.admin.unwrap_or(updating.admin),
        &mut pool,
    )
    .await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: result,
    }))
}
