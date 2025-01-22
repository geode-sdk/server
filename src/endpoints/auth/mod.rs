use crate::config::AppData;
use crate::database::repository::{auth_tokens, developers, refresh_tokens};
use crate::extractors::auth::Auth;
use crate::types::api::{ApiError, ApiResponse};
use actix_web::{post, web, Responder};
use serde::{Deserialize, Serialize};
use sqlx::Acquire;
use uuid::Uuid;

pub mod github;

#[derive(Serialize)]
struct TokensResponse {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct RefreshBody {
    refresh_token: String,
}

#[post("v1/login/refresh")]
pub async fn refresh_token(
    json: web::Json<RefreshBody>,
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let auth_token = auth.token().ok();

    let refresh_token = Uuid::parse_str(&json.refresh_token)
        .or(Err(ApiError::BadRequest("Invalid refresh token".into())))?;

    let mut conn = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let found = developers::find_by_refresh_token(refresh_token, &mut conn)
        .await?
        .ok_or(ApiError::BadRequest(
            "Invalid or expired refresh token".into(),
        ))?;

    let mut tx = conn.begin().await.or(Err(ApiError::TransactionError))?;

    let new_auth = auth_tokens::generate_token(found.id, true, &mut tx).await?;
    let new_refresh = refresh_tokens::generate_token(found.id, &mut tx).await?;

    if let Some(auth) = auth_token {
        auth_tokens::remove_token(auth, &mut tx).await?;
    }
    refresh_tokens::remove_token(refresh_token, &mut tx).await?;

    tx.commit().await.or(Err(ApiError::TransactionError))?;

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: TokensResponse {
            access_token: new_auth.to_string(),
            refresh_token: Some(new_refresh.to_string()),
        },
    }))
}
