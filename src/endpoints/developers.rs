use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use sqlx::Acquire;

use crate::{
    auth::token,
    extractors::auth::Auth,
    types::{
        api::{ApiError, ApiResponse},
        models::{
            developer::{Developer, DeveloperProfile},
            mod_entity::Mod,
            mod_version_status::ModVersionStatusEnum,
        },
    },
    AppData,
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SimpleDevMod {
    pub id: String,
    pub featured: bool,
    pub download_count: i32,
    pub versions: Vec<SimpleDevModVersion>,
    pub developers: Vec<Developer>,
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let mut page = query.page.unwrap_or(1);
    if page < 1 {
        page = 1
    }
    let mut per_page = query.per_page.unwrap_or(15);
    if per_page < 1 {
        per_page = 1
    }
    if per_page > 100 {
        per_page = 100
    }

    let result = Developer::get_index(&query.query, page, per_page, &mut pool).await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: result,
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if !(Developer::owns_mod(dev.id, &path.id, &mut transaction).await?) {
        return Err(ApiError::Forbidden);
    }
    let dev = match Developer::find_by_username(&json.username, &mut transaction).await? {
        None => {
            return Err(ApiError::BadRequest(format!(
                "No developer found with username {}",
                json.username
            )))
        }
        Some(d) => d,
    };

    if (Mod::get_one(&path.id, false, &mut transaction).await?).is_none() {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }

    if let Err(e) = Mod::assign_dev(&path.id, dev.id, &mut transaction).await {
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

#[delete("v1/mods/{id}/developers/{username}")]
pub async fn remove_dev_from_mod(
    data: web::Data<AppData>,
    path: web::Path<RemoveDevPath>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if !(Developer::owns_mod(dev.id, &path.id, &mut transaction).await?) {
        return Err(ApiError::Forbidden);
    }
    let dev = match Developer::find_by_username(&path.username, &mut transaction).await? {
        None => {
            return Err(ApiError::BadRequest(format!(
                "No developer found with username {}",
                path.username
            )))
        }
        Some(d) => d,
    };
    if (Mod::get_one(&path.id, false, &mut transaction).await?).is_none() {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }

    if let Err(e) = Mod::unassign_dev(&path.id, dev.id, &mut transaction).await {
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

#[delete("v1/me/token")]
pub async fn delete_token(
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let token = auth.token()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if let Err(e) =
        token::invalidate_token_for_developer(dev.id, token.to_string(), &mut transaction).await
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

#[delete("v1/me/tokens")]
pub async fn delete_tokens(
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if let Err(e) = token::invalidate_tokens_for_developer(dev.id, &mut transaction).await {
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    if let Err(e) = Developer::update_profile(dev.id, &json.display_name, &mut transaction).await {
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
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
        payload: DeveloperProfile {
            id: dev.id,
            username: dev.username,
            display_name: dev.display_name,
            verified: dev.verified,
            admin: dev.admin,
        },
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
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let result = Developer::get_one(path.id, &mut pool).await?;

    if result.is_none() {
        return Err(ApiError::NotFound("Developer not found".to_string()));
    }

    let result = result.unwrap();
    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: DeveloperProfile {
            id: result.id,
            username: result.username,
            display_name: result.display_name,
            verified: result.verified,
            admin: result.admin,
        },
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
    if !dev.admin {
        return Err(ApiError::Forbidden);
    }

    if payload.admin.is_none() && payload.verified.is_none() {
        return Ok(HttpResponse::Ok());
    }

    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    if payload.admin.is_some() && dev.id == path.id {
        return Err(ApiError::BadRequest(
            "Can't override your own admin status".to_string(),
        ));
    }

    if let Err(api_err) =
        Developer::update(path.id, payload.admin, payload.verified, &mut transaction).await
    {
        if let Err(e) = transaction.rollback().await {
            log::error!("{}", e);
        }
        return Err(api_err);
    }
    if let Err(e) = transaction.commit().await {
        log::error!("{}", e);
        return Err(ApiError::DbError);
    }
    Ok(HttpResponse::Ok())
}
