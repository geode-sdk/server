use actix_web::{delete, post, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Acquire;

use crate::{
    auth::token,
    extractors::auth::Auth,
    types::{
        api::ApiError,
        models::{developer::Developer, mod_entity::Mod},
    },
    AppData,
};

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

#[post("v1/mods/{id}/developers")]
pub async fn add_developer_to_mod(
    data: web::Data<AppData>,
    path: web::Path<AddDevPath>,
    json: web::Json<AddDevPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.into_developer()?;
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

    if (Mod::get_one(&path.id, &mut transaction).await?).is_none() {
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
    let dev = auth.into_developer()?;
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
    if (Mod::get_one(&path.id, &mut transaction).await?).is_none() {
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

#[delete("v1/me/tokens")]
pub async fn delete_tokens(
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.into_developer()?;
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
