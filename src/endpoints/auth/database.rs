use actix_web::{Responder, post, web};
use argon2::{PasswordHash, PasswordVerifier};
use serde::Deserialize;
use sqlx::Connection;
use utoipa::ToSchema;
use validator::Validate;

use crate::{
    auth,
    config::AppData,
    database::repository::{auth_tokens, developers, refresh_tokens},
    endpoints::{ApiError, auth::TokensResponse},
    types::api::ApiResponse,
};

#[derive(ToSchema, Validate, Deserialize)]
struct EmailLogin {
    #[validate(email)]
    email: String,
    password: String,
}

/// Login using email
#[utoipa::path(
    post,
    path = "/v1/login",
    tag = "auth",
    responses(
        (status = 200, description = "Authenticated", body = inline(ApiResponse<TokensResponse>)),
        (status = 401, description = "Invalid login"),
        (status = 500, description = "Server error")
    )
)]
#[post("v1/login")]
#[tracing::instrument(skip_all)]
pub async fn login(
    data: web::Data<AppData>,
    json: web::Json<EmailLogin>,
) -> Result<impl Responder, ApiError> {
    let login_data = {
        let mut pool = data.db().acquire().await?;

        developers::find_login_data_by_email(&json.email, &mut pool)
            .await?
            .ok_or(ApiError::BadRequest(
                "No user found with these credentials".into(),
            ))?
    };

    let password_hash = login_data.password_hash.clone();
    let pepper = data.password_hash_pepper().cloned();

    tokio::task::spawn_blocking(move || {
        let parsed_hash = PasswordHash::new(&password_hash)
            .inspect_err(|e| tracing::error!("failed to parse password hash - pretty bad: {e}"))
            .map_err(|_| ApiError::InternalError("couldn't read user password".into()))?;

        auth::password::argon2(pepper.as_ref())
            .verify_password(json.password.as_bytes(), &parsed_hash)
            .map_err(|_| ApiError::BadRequest("No user found with these credentials".into()))
    })
    .await
    .inspect_err(|e| tracing::error!("spawn_blocking failed: {e}"))
    .map_err(|_| ApiError::InternalError("hash check task panicked".into()))??;

    let mut pool = data.db().acquire().await?;
    let mut tx = pool.begin().await?;

    let token = auth_tokens::generate_token(login_data.id, true, &mut tx).await?;
    let refresh = refresh_tokens::generate_token(login_data.id, &mut tx).await?;

    tx.commit().await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: TokensResponse {
            access_token: token.to_string(),
            refresh_token: refresh.to_string(),
        },
    }))
}
