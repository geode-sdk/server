use crate::{
    config::AppData,
    database::repository::{deprecations, developers, mods},
    endpoints::ApiError,
    extractors::auth::Auth,
    types::api::ApiResponse,
};
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;
use sqlx::{Acquire, PgConnection};

#[derive(Deserialize)]
struct ModPath {
    id: String,
}

#[derive(Deserialize)]
struct ModDeprecationPath {
    id: String,
    deprecation_id: i32,
}

#[derive(Deserialize)]
struct CreateDeprecationData {
    by: Vec<String>,
    reason: String,
}

#[derive(Deserialize)]
struct UpdateDeprecationData {
    by: Option<Vec<String>>,
    reason: Option<String>,
}

const MAX_MODS_PER_DEPRECATION: usize = 20;

/// Fetch all deprecations for a mod
#[get("v1/mods/{id}/deprecations")]
pub async fn index(
    data: web::Data<AppData>,
    path: web::Path<ModPath>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }
    // Allow admins to deprecate any mod
    if !dev.admin && !developers::owns_mod(dev.id, &path.id, &mut pool).await? {
        return Err(ApiError::Authorization);
    }

    let deps = deprecations::get_for_mods(std::slice::from_ref(&path.id), &mut pool).await?;
    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: deps,
    }))
}

/// Insert one deprecation for a mod
#[post("v1/mods/{id}/deprecations")]
pub async fn store(
    data: web::Data<AppData>,
    path: web::Path<ModPath>,
    json: web::Json<CreateDeprecationData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;
    let mut tx = pool.begin().await?;

    if !mods::exists(&path.id, &mut tx).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }

    if !dev.admin && !developers::owns_mod(dev.id, &path.id, &mut tx).await? {
        return Err(ApiError::Authorization);
    }

    check_existing_mods(&json.by, &mut tx).await?;

    let deprecation = deprecations::create(&path.id, &json.by, &json.reason, &dev, &mut tx).await?;

    tx.commit().await?;
    Ok(HttpResponse::Created().json(ApiResponse {
        error: "".into(),
        payload: deprecation,
    }))
}

/// Update a deprecation
#[put("v1/mods/{id}/deprecations/{deprecation_id}")]
pub async fn update(
    data: web::Data<AppData>,
    path: web::Path<ModDeprecationPath>,
    json: web::Json<UpdateDeprecationData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;
    let mut tx = pool.begin().await?;

    if !mods::exists(&path.id, &mut tx).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }

    if let Some(by) = &json.by {
        check_existing_mods(by, &mut tx).await?;
    }

    let deprecation = deprecations::get(path.deprecation_id, &mut tx)
        .await?
        .ok_or(ApiError::NotFound(format!(
            "Deprecation id {} not found",
            path.deprecation_id
        )))?;

    // If the ID doesn't match, just ignore it
    if deprecation.mod_id != path.id {
        return Err(ApiError::NotFound(format!(
            "Deprecation id {} not found",
            path.deprecation_id
        )));
    }

    if !dev.admin && !developers::owns_mod(dev.id, &path.id, &mut tx).await? {
        return Err(ApiError::Authorization);
    }

    let updated = deprecations::update(
        deprecation,
        json.by.as_deref(),
        json.reason.as_deref(),
        &dev,
        &mut tx,
    )
    .await?;

    tx.commit().await?;
    Ok(HttpResponse::Ok().json(ApiResponse {
        error: "".into(),
        payload: updated,
    }))
}

/// Update a deprecation
#[delete("v1/mods/{id}/deprecations/{deprecation_id}")]
pub async fn delete(
    data: web::Data<AppData>,
    path: web::Path<ModDeprecationPath>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }

    let deprecation = deprecations::get(path.deprecation_id, &mut pool)
        .await?
        .ok_or(ApiError::NotFound(format!(
            "Deprecation id {} not found",
            path.deprecation_id
        )))?;

    // If the ID doesn't match, just ignore it
    if deprecation.mod_id != path.id {
        return Err(ApiError::NotFound(format!(
            "Deprecation id {} not found",
            path.deprecation_id
        )));
    }

    if !dev.admin && !developers::owns_mod(dev.id, &path.id, &mut pool).await? {
        return Err(ApiError::Authorization);
    }

    deprecations::delete(deprecation.id, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}

#[delete("v1/mods/{id}/deprecations")]
pub async fn clear_all(
    data: web::Data<AppData>,
    path: web::Path<ModPath>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }

    if !dev.admin && !developers::owns_mod(dev.id, &path.id, &mut pool).await? {
        return Err(ApiError::Authorization);
    }

    deprecations::clear_all(&path.id, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}

async fn check_existing_mods(ids: &[String], conn: &mut PgConnection) -> Result<(), ApiError> {
    if ids.len() > MAX_MODS_PER_DEPRECATION {
        return Err(ApiError::BadRequest(format!(
            "Max {} mods allowed per deprecation",
            MAX_MODS_PER_DEPRECATION
        )));
    }

    let (_, missing) = mods::exists_multiple(ids, &mut *conn).await?;

    if missing.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "The following mods don't exist on the index: {}",
            missing.join(", ")
        )));
    }

    Ok(())
}
