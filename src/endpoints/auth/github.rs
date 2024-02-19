use actix_web::{dev::ConnectionInfo, post, web, Responder};
use serde::Deserialize;
use sqlx::types::ipnetwork::{IpNetwork, Ipv4Network};
use uuid::Uuid;

use crate::{
    auth::{github, token::create_token_for_developer},
    types::{
        api::{ApiError, ApiResponse},
        models::{developer::Developer, github_login_attempt::GithubLoginAttempt},
    },
    AppData,
};

#[derive(Deserialize)]
struct PollParams {
    uuid: String,
}

#[post("v1/login/github")]
pub async fn start_github_login(
    data: web::Data<AppData>,
    info: ConnectionInfo,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let client = github::GithubClient::new(
        data.github_client_id.to_string(),
        data.github_client_secret.to_string(),
    );
    let ip = match info.realip_remote_addr() {
        None => return Err(ApiError::InternalError),
        Some(i) => i,
    };
    log::info!("{}", ip);
    let net: IpNetwork = ip.parse().or(Err(ApiError::InternalError))?;

    let result = client.start_auth(net, &mut pool).await?;
    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: result,
    }))
}

#[post("v1/login/github/poll")]
pub async fn poll_github_login(
    json: web::Json<PollParams>,
    data: web::Data<AppData>,
    connection_info: ConnectionInfo,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let uuid = match Uuid::parse_str(&json.uuid) {
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::BadRequest(format!("Invalid uuid {}", json.uuid)));
        }
        Ok(u) => u,
    };
    let attempt = match GithubLoginAttempt::get_one(uuid, &mut pool).await? {
        None => {
            return Err(ApiError::BadRequest(format!(
                "No attempt made for uuid {}",
                json.uuid
            )))
        }
        Some(a) => a,
    };

    let ip = match connection_info.realip_remote_addr() {
        None => return Err(ApiError::InternalError),
        Some(i) => i,
    };
    let net: Ipv4Network = ip.parse().or(Err(ApiError::InternalError))?;
    if attempt.ip.ip() != net.ip() {
        log::error!("{} compared to {}", attempt.ip, net);
        return Err(ApiError::BadRequest(
            "Request IP does not match stored attempt IP".to_string(),
        ));
    }
    if !attempt.interval_passed() {
        return Err(ApiError::BadRequest("Too fast".to_string()));
    }
    if attempt.is_expired() {
        GithubLoginAttempt::remove(uuid, &mut pool).await;
        return Err(ApiError::BadRequest("Login attempt expired".to_string()));
    }

    let client = github::GithubClient::new(
        data.github_client_id.to_string(),
        data.github_client_secret.to_string(),
    );
    GithubLoginAttempt::poll(uuid, &mut pool).await;
    let token = client.poll_github(&attempt.device_code).await?;
    GithubLoginAttempt::remove(uuid, &mut pool).await;
    let user = client.get_user(token).await?;
    let id = match user.get("id") {
        None => return Err(ApiError::InternalError),
        Some(id) => id.as_i64().unwrap(),
    };
    if let Some(x) = Developer::get_by_github_id(id, &mut pool).await? {
        let token = create_token_for_developer(x.id, &mut pool).await?;
        return Ok(web::Json(ApiResponse {
            error: "".to_string(),
            payload: token.to_string(),
        }));
    }
    let username = match user.get("login") {
        None => return Err(ApiError::InternalError),
        Some(user) => user.to_string(),
    };
    let id = Developer::create(id, username, &mut pool).await?;
    let token = create_token_for_developer(id, &mut pool).await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: token.to_string(),
    }))
}
