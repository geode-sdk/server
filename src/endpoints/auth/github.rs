use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;
use sqlx::types::ipnetwork::Ipv4Network;

use crate::{auth::github, types::api::{ApiError, ApiResponse}, AppData};

#[derive(Deserialize)]
struct PollParams {
    device_code: String
}

#[post("v1/login/github")]
pub async fn start_github_login(data: web::Data<AppData>, req: HttpRequest) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let client = github::GithubClient::new(data.github_client_id.to_string(), data.github_client_secret.to_string());
    let connection_info = req.connection_info();
    let ip = match connection_info.realip_remote_addr() {
        None => return Err(ApiError::InternalError),
        Some(i) => i
    };
    log::info!("{}", ip);
    let net: Ipv4Network = ip.parse().or(Err(ApiError::InternalError))?;

    let result = client.start_auth(net, &mut *pool).await?;
    Ok(web::Json(ApiResponse {error: "".to_string(), payload: result}))
}

#[post("v1/login/github/poll")]
pub async fn poll_github_login(json: web::Json<PollParams>, data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let client = github::GithubClient::new(data.github_client_id.to_string(), data.github_client_secret.to_string());
    let result = client.poll_github(&json.device_code).await?;
    Ok(web::Json(ApiResponse {error: "".to_string(), payload: result}))
}