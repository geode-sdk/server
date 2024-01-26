use actix_web::{get, post, Responder};

use crate::types::api::ApiError;

#[post("v1/login/github")]
pub async fn start_github_login() -> Result<impl Responder, ApiError> {
    Ok("")
}

#[get("v1/login/github/poll")]
pub async fn poll_github_login() -> Result<impl Responder, ApiError> {
    Ok("")
}