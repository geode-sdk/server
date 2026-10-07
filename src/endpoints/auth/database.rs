use std::{str::FromStr, sync::OnceLock};

use actix_web::{Responder, post, web};
use argon2::{PasswordHash, PasswordHasher, PasswordVerifier};
use serde::Deserialize;
use sqlx::Connection;
use utoipa::ToSchema;
use validator::Validate;

use crate::{
    auth,
    config::AppData,
    database::repository::{auth_tokens, developers, refresh_tokens},
    email::EmailAddress,
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

        let email = EmailAddress::from_str(&json.email)
            .map_err(|_| ApiError::BadRequest("invalid email address".into()))?
            .to_string();

        developers::find_login_data_by_email(&email, &mut pool).await?
    };

    let password_hash = login_data.as_ref().map(|data| &data.password_hash).cloned();
    let pepper = data.password_hash_pepper().cloned();

    tokio::task::spawn_blocking(move || {
        // Run the hash check anyway to reduce timing attack probability.
        // Of course, someone putting in "soggymod" would pass this (if no dev exists with given email)
        // but will fail when login data is unwrapped.
        let password_hash = password_hash.unwrap_or_else(|| dummy_hash(pepper.as_ref()).to_owned());

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

    let login_data = login_data.ok_or(ApiError::BadRequest(
        "No user found with these credentials".into(),
    ))?;

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

fn dummy_hash(pepper: Option<&String>) -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();

    HASH.get_or_init(|| {
        auth::password::argon2(pepper)
            .hash_password(b"soggymod")
            .expect("hashing a static str can't fail")
            .to_string()
    })
}
