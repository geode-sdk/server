use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Acquire;

use crate::{
    extractors::auth::Auth,
    types::{api::ApiError, models::rejection},
    AppData,
};

#[derive(Deserialize)]
struct RejectModPath {
    id: String,
    version: String,
}

#[derive(Deserialize)]
struct RejectModPayload {
    reason: Option<String>,
}

#[post("v1/mods/{id}/version/{version}/rejections")]
pub async fn reject_mod(
    path: web::Path<RejectModPath>,
    auth: Auth,
    payload: web::Json<RejectModPayload>,
    data: web::Data<AppData>,
) -> Result<impl Responder, ApiError> {
    let developer = auth.developer()?;
    if !developer.admin {
        return Err(ApiError::Forbidden);
    }

    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let version = match semver::Version::parse(&path.version) {
        Ok(version) => version,
        Err(_) => return Err(ApiError::NotFound("".to_string())),
    };

    if let Err(e) = rejection::reject_mod(
        &path.id,
        version,
        payload.reason.clone(),
        &developer,
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
