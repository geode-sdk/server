use actix_web::{get, post, delete, web, HttpResponse, Responder};
use serde::{Deserialize};
use sqlx::Acquire;

use crate::{
    extractors::auth::Auth,
    types::{
        api::{ApiError, ApiResponse},
        models::{
            developer::{Developer},
        },
    },
    AppData
};
use crate::types::models::mod_version::ModVersion;
use crate::types::models::mod_feedback::{ModFeedback,FeedbackTypeEnum};

#[derive(Deserialize)]
pub struct GetModFeedbackPath {
    id: String,
    version: String
}

#[derive(Deserialize)]
pub struct PostModFeedbackPayload {
    feedback_type: FeedbackTypeEnum,
    feedback: String,
}

#[derive(Deserialize)]
pub struct DeleteModFeedbackPayload {
    id: i32
}

#[get("/v1/mods/{id}/versions/{version}/feedback")]
pub async fn get_mod_feedback(
    data: web::Data<AppData>,
    path: web::Path<GetModFeedbackPath>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let access = Developer::has_access_to_mod(dev.id, &path.id, &mut pool).await?;

    if !access && !dev.admin && !dev.verified {
        return Err(ApiError::Forbidden);
    }

    let note_only = !access && !dev.admin;

    let mod_version = {
        if path.version == "latest" {
            ModVersion::get_latest_for_mod(&path.id, None, vec![], None, &mut pool).await?
        } else {
            ModVersion::get_one(path.id.strip_prefix('v').unwrap_or(&path.id), &path.version, false, false, &mut pool).await?
        }
    };

    let feedback = ModFeedback::get_for_mod_version_id(&mod_version, note_only, &mut pool).await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: feedback,
    }))
}

#[post("/v1/mods/{id}/versions/{version}/feedback")]
pub async fn post_mod_feedback(
    data: web::Data<AppData>,
    path: web::Path<GetModFeedbackPath>,
    payload: web::Json<PostModFeedbackPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let access = Developer::has_access_to_mod(dev.id, &path.id, &mut transaction).await?;

    if !access && !dev.verified && !dev.admin {
        return Err(ApiError::Forbidden);
    }

    if access && payload.feedback_type != FeedbackTypeEnum::Note {
        return Err(ApiError::Forbidden);
    }

    let mod_version = {
        if path.version == "latest" {
            ModVersion::get_latest_for_mod(&path.id, None, vec![], None, &mut transaction).await?
        } else {
            ModVersion::get_one(path.id.strip_prefix('v').unwrap_or(&path.id), &path.version, false, false, &mut transaction).await?
        }
    };

    let result = ModFeedback::set(&mod_version, dev.id, payload.feedback_type.clone(), &payload.feedback, false, access, &mut transaction).await;

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

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: result?,
    }))
}

#[delete("/v1/mods/{id}/versions/{version}/feedback")]
pub async fn delete_mod_feedback(
    data: web::Data<AppData>,
    path: web::Path<GetModFeedbackPath>,
    payload: web::Json<DeleteModFeedbackPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    if !dev.admin {
        let feedback = ModFeedback::get_feedback_by_id(payload.id, &mut transaction).await?;
        if feedback.reviewer.id != dev.id {
            return Err(ApiError::Forbidden);
        }
    }

    let result = ModFeedback::remove(payload.id, &mut transaction).await;

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