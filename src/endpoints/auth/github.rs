use actix_web::{dev::ConnectionInfo, post, web, Responder};
use serde::Deserialize;
use sqlx::{types::ipnetwork::IpNetwork, Acquire};
use uuid::Uuid;

use crate::database::repository::{auth_tokens, developers};
use crate::{
    auth::github,
    types::{
        api::{ApiError, ApiResponse},
        models::github_login_attempt::GithubLoginAttempt,
    },
    AppData,
};

#[derive(Deserialize)]
struct PollParams {
    uuid: String,
}

#[derive(Deserialize)]
struct TokenLoginParams {
    token: String,
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
    let attempt =
        GithubLoginAttempt::get_one(uuid, &mut pool)
            .await?
            .ok_or(ApiError::BadRequest(
                "No login attempt has been made for this UUID".into(),
            ))?;

    let net: IpNetwork = connection_info
        .realip_remote_addr()
        .ok_or(ApiError::BadRequest(
            "No IP address detected from request".into(),
        ))?
        .parse()
        .or(Err(ApiError::BadRequest(
            "Failed to parse IP address from request".into(),
        )))?;

    if attempt.ip.ip() != net.ip() {
        return Err(ApiError::BadRequest(
            "IP address does not match stored login attempt IP address".into(),
        ));
    }

    if !attempt.interval_passed() {
        return Err(ApiError::BadRequest("Too fast".into()));
    }

    if attempt.is_expired() {
        GithubLoginAttempt::remove(uuid, &mut pool).await?;
        return Err(ApiError::BadRequest("Login attempt expired".to_string()));
    }

    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let client = github::GithubClient::new(
        data.github_client_id.to_string(),
        data.github_client_secret.to_string(),
    );
    GithubLoginAttempt::poll(uuid, &mut tx).await;
    let token = client.poll_github(&attempt.device_code).await?;
    GithubLoginAttempt::remove(uuid, &mut tx).await?;

    // Create a new transaction after this point, because we need to commit the removal of the login attempt
    // It would be invalid for GitHub anyway

    tx.commit().await.or(Err(ApiError::TransactionError))?;

    let user = client
        .get_user(&token)
        .await
        .map_err(|_| ApiError::InternalError)?;

    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let developer = developers::fetch_or_insert_github(user.id, &user.username, &mut tx).await?;
    let token = auth_tokens::generate_token(developer.id, &mut tx).await?;

    tx.commit().await.or(Err(ApiError::TransactionError))?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: token.to_string(),
    }))
}

#[post("v1/login/github/token")]
pub async fn github_token_login(
    json: web::Json<TokenLoginParams>,
    data: web::Data<AppData>,
) -> Result<impl Responder, ApiError> {
    let client = github::GithubClient::new(
        data.github_client_id.to_string(),
        data.github_client_secret.to_string(),
    );

    let user = client
        .get_user(&json.token)
        .await
        .map_err(|_| ApiError::BadRequest(format!("Invalid access token: {}", json.token)))?;

    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let developer = developers::fetch_or_insert_github(user.id, &user.username, &mut tx).await?;
    let token = auth_tokens::generate_token(developer.id, &mut tx).await?;

    tx.commit().await.or(Err(ApiError::TransactionError))?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: token.to_string(),
    }))
}
