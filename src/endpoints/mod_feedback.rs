use actix_web::{get, post, web, HttpResponse, Responder};
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
    AppData,
    webhook::send_webhook
};
use crate::types::models::mod_version::ModVersion;
use crate::types::models::mod_feedback::ModFeedback;
use crate::types::models::mod_version_status::ModVersionStatusEnum;

#[derive(Deserialize)]
pub struct GetModFeedbackPath {
    id: String,
    version: String
}

#[derive(Deserialize)]
pub struct PostModFeedbackPayload {
    positive: bool,
    feedback: String,
    decision: Option<bool>, // Admin Only: Setting this will turn the request into a decision, like PUT v1/mods/{id}/versions/{version}
}

#[get("/v1/mods/{id}/versions/{version}/feedback")]
pub async fn get_mod_feedback(
    data: web::Data<AppData>,
    path: web::Path<GetModFeedbackPath>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    if !Developer::has_access_to_mod(dev.id, &path.id, &mut pool).await? && !dev.admin {
        return Err(ApiError::Forbidden);
    }

    let mod_version = {
        if path.version == "latest" {
            ModVersion::get_latest_for_mod(&path.id, None, vec![], None, &mut pool).await?
        } else {
            ModVersion::get_one(&path.id, &path.version, false, false, &mut pool).await?
        }
    };

    let feedback = ModFeedback::get_for_mod_version_id(&mod_version, &mut pool).await?;

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

    if (!dev.verified && !dev.admin) || Developer::has_access_to_mod(dev.id, &path.id, &mut transaction).await? {
        return Err(ApiError::Forbidden);
    }

    let decision = payload.decision.unwrap_or(false);
    let mut status = None;
    if decision {
        if !dev.admin {
            return Err(ApiError::Forbidden);
        }
        status = Some(match payload.positive {
            true => ModVersionStatusEnum::Accepted,
            false => ModVersionStatusEnum::Rejected,
        });
    }

    let mod_version = {
        if path.version == "latest" {
            ModVersion::get_latest_for_mod(&path.id, None, vec![], None, &mut transaction).await?
        } else {
            ModVersion::get_one(&path.id, &path.version, false, false, &mut transaction).await?
        }
    };

    let result = ModFeedback::set(&mod_version, dev.id, payload.positive, &payload.feedback, decision, &mut transaction).await;

    if result.is_err() {
        transaction
            .rollback()
            .await
            .or(Err(ApiError::TransactionError))?;
        return Err(result.err().unwrap());
    }

    if let Some(status) = status {
        if let Err(e) = ModVersion::update_version(
            mod_version.id,
            status,
            payload.feedback.clone().into(),
            dev.id,
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

        if status == ModVersionStatusEnum::Accepted {
            let approved_count = ModVersion::get_accepted_count(mod_version.mod_id.as_str(), &mut transaction).await?;

            let is_update = approved_count > 0;

            let owner = Developer::fetch_for_mod(path.id.as_str(), &mut transaction)
                .await?
                .into_iter()
                .find(|dev| dev.is_owner);

            send_webhook(
                mod_version.mod_id,
                mod_version.name.clone(),
                mod_version.version.clone(),
                is_update,
                owner.as_ref().unwrap().clone(),
                dev.clone(),
                data.webhook_url.clone(),
                data.app_url.clone()
            ).await;
        }
    }

    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    Ok(HttpResponse::NoContent())
}