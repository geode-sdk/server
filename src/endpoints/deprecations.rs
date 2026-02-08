
use actix_web::{HttpResponse, Responder, get, post, web};
use serde::Deserialize;
use crate::{
    config::AppData,
    database::repository::{developers, mods},
    endpoints::ApiError,
    extractors::auth::Auth,
    types::{api::ApiResponse, models::deprecations::Deprecation}
};

#[derive(Deserialize)]
struct DeprecationPath {
    id: String,
}

#[derive(Deserialize)]
struct UpdateDeprecationParams {
    by: Vec<String>,
    reason: String,
}

/// Update the deprecation status of a mod
#[get("v1/mods/{id}/deprecate")]
pub async fn check_mod_deprecation(
    data: web::Data<AppData>,
    path: web::Path<DeprecationPath>,
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

    let deps = Deprecation::get_deprecations_for(&mut pool, std::slice::from_ref(&path.id)).await?;
    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: deps.into_iter().next()
    }))
}

/// Update the deprecation status of a mod
#[post("v1/mods/{id}/deprecate")]
pub async fn deprecate_mod(
    data: web::Data<AppData>,
    path: web::Path<DeprecationPath>,
    json: web::Json<Option<UpdateDeprecationParams>>,
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

    // Delete old deprecation data (if it exists)
    // If we're updating deprecations, just replace the data
    Deprecation::delete_deprecation(&mut pool, &path.id).await?;
    
    // Add new deprecation data if provided
    if let Some(json) = json.0 {
        Deprecation::create_deprecation(&mut pool, &path.id, &json.by, &json.reason).await?;
    }

    Ok(HttpResponse::NoContent())
}

