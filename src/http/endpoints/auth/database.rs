use std::str::FromStr;

use actix_web::{Responder, post, web};
use serde::Deserialize;
use sqlx::Connection;
use utoipa::ToSchema;
use validator::Validate;

use crate::{
    auth,
    config::AppData,
    database::repository::{auth_tokens, developers, refresh_tokens},
    email::EmailAddress,
    http::{endpoints::auth::TokensResponse, error::ApiError, response::ApiResponse},
};

pub fn router_config(cfg: &mut web::ServiceConfig) {
    cfg.service(login).service(forgot_password);
}

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
#[post("/login")]
#[tracing::instrument(skip_all)]
pub async fn login(
    data: web::Data<AppData>,
    json: web::Json<EmailLogin>,
) -> Result<impl Responder, ApiError> {
    data.mailer().ok_or(ApiError::NotImplemented)?;

    let login_data = {
        let mut pool = data.db().acquire().await?;

        let email = EmailAddress::from_str(&json.email)
            .map_err(|_| ApiError::BadRequest("invalid email address".into()))?
            .to_string();

        developers::find_login_data_by_email(&email, &mut pool).await?
    };

    let password_hash = login_data.as_ref().map(|data| &data.password_hash).cloned();
    let pepper = data.password_hash_pepper().cloned();

    if !auth::password::verify_or_dummy(password_hash, json.password.clone(), pepper).await? {
        return Err(ApiError::BadRequest(
            "No user found with these credentials".into(),
        ));
    }

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

#[derive(ToSchema, Deserialize, Validate)]
struct ForgotPassword {
    #[validate(email)]
    email: String,
}

/// Start forgot password flow
#[utoipa::path(
    tag = "auth",
    responses(
        (status = 204, description = "Maybe sent password reset notification"),
        (status = 500, description = "Server error")
    )
)]
#[tracing::instrument(skip_all)]
#[post("/forgot-password")]
pub async fn forgot_password(
    data: web::Data<AppData>,
    json: web::Json<ForgotPassword>,
) -> Result<impl Responder, ApiError> {
    data.mailer().ok_or(ApiError::NotImplemented)?;
    let front_url = data.front_url();
    if front_url.is_empty() {
        return Err(ApiError::NotImplemented);
    }

    let developer = {
        let mut conn = data.db().acquire().await?;

        let email = EmailAddress::from_str(&json.email.to_lowercase())
            .map_err(|_| ApiError::BadRequest("invalid email address".into()))?
            .to_string();

        developers::find_by_email(&email, &mut conn).await?
    };

    Ok(web::Json({}))
}
