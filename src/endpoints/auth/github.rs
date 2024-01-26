use actix_web::{get, post, web, Responder};
use serde::Deserialize;

use crate::{auth::github, types::api::{ApiError, ApiResponse}, AppData};

#[derive(Deserialize)]
struct PollParams {
    device_code: String
}

#[post("v1/login/github")]
pub async fn start_github_login(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let result = github::start_auth(&data.github_client_id).await?;
    Ok(web::Json(ApiResponse {error: "".to_string(), payload: result}))
}

#[post("v1/login/github/poll")]
pub async fn poll_github_login(json: web::Json<PollParams>, data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let result = github::poll_github(&data.github_client_id, &json.device_code).await?;
    Ok(web::Json(ApiResponse {error: "".to_string(), payload: result}))
}