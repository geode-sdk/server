use actix_web::{dev::ConnectionInfo, post, web, Responder};
use serde::Deserialize;
use sqlx::{
    migrate::Migrate,
    types::ipnetwork::{IpNetwork, Ipv4Network},
    Acquire,
};
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
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
    let uuid = match Uuid::parse_str(&json.uuid) {
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::BadRequest(format!("Invalid uuid {}", json.uuid)));
        }
        Ok(u) => u,
    };
    let attempt = match GithubLoginAttempt::get_one(uuid, &mut transaction).await? {
        None => {
            transaction
                .rollback()
                .await
                .or(Err(ApiError::TransactionError))?;
            return Err(ApiError::BadRequest(format!(
                "No attempt made for uuid {}",
                json.uuid
            )));
        }
        Some(a) => a,
    };

    let ip = match connection_info.realip_remote_addr() {
        None => return Err(ApiError::InternalError),
        Some(i) => i,
    };
    let net: Ipv4Network = match ip.parse() {
        Err(e) => {
            transaction
                .rollback()
                .await
                .or(Err(ApiError::TransactionError))?;
            log::error!("{}", e);
            return Err(ApiError::BadRequest("Invalid IP".to_string()));
        }
        Ok(n) => n,
    };
    if attempt.ip.ip() != net.ip() {
        transaction
            .rollback()
            .await
            .or(Err(ApiError::TransactionError))?;
        log::error!("{} compared to {}", attempt.ip, net);
        return Err(ApiError::BadRequest(
            "Request IP does not match stored attempt IP".to_string(),
        ));
    }
    if !attempt.interval_passed() {
        transaction
            .rollback()
            .await
            .or(Err(ApiError::TransactionError))?;
        return Err(ApiError::BadRequest("Too fast".to_string()));
    }
    if attempt.is_expired() {
        match GithubLoginAttempt::remove(uuid, &mut transaction).await {
            Err(e) => {
                transaction
                    .rollback()
                    .await
                    .or(Err(ApiError::TransactionError))?;
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            }
            Ok(_) => {
                transaction
                    .commit()
                    .await
                    .or(Err(ApiError::TransactionError))?;
                return Err(ApiError::BadRequest("Login attempt expired".to_string()));
            }
        };
    }

    let client = github::GithubClient::new(
        data.github_client_id.to_string(),
        data.github_client_secret.to_string(),
    );
    GithubLoginAttempt::poll(uuid, &mut transaction).await;
    let token = client.poll_github(&attempt.device_code).await?;
    if let Err(e) = GithubLoginAttempt::remove(uuid, &mut transaction).await {
        transaction
            .rollback()
            .await
            .or(Err(ApiError::TransactionError))?;
        log::error!("{}", e);
        return Err(ApiError::InternalError);
    };
    let user = match client.get_user(token).await {
        Err(e) => {
            transaction
                .rollback()
                .await
                .or(Err(ApiError::TransactionError))?;
            log::error!("{}", e);
            return Err(ApiError::InternalError);
        }
        Ok(u) => u,
    };

    // Create a new transaction after this point, because we need to commit the removal of the login attempt

    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let id = match user.get("id") {
        None => return Err(ApiError::InternalError),
        Some(id) => id.as_i64().unwrap(),
    };
    if let Some(x) = Developer::get_by_github_id(id, &mut transaction).await? {
        let token = match create_token_for_developer(x.id, &mut transaction).await {
            Err(_) => {
                transaction
                    .rollback()
                    .await
                    .or(Err(ApiError::TransactionError))?;
                return Err(ApiError::InternalError);
            }
            Ok(t) => t,
        };
        transaction
            .commit()
            .await
            .or(Err(ApiError::TransactionError))?;
        return Ok(web::Json(ApiResponse {
            error: "".to_string(),
            payload: token.to_string(),
        }));
    }
    let username = match user.get("login") {
        None => {
            transaction
                .rollback()
                .await
                .or(Err(ApiError::TransactionError))?;
            return Err(ApiError::InternalError);
        }
        Some(user) => user.to_string(),
    };
    let id = match Developer::create(id, username, &mut transaction).await {
        Err(e) => {
            transaction
                .rollback()
                .await
                .or(Err(ApiError::TransactionError))?;
            log::error!("{}", e);
            return Err(ApiError::InternalError);
        }
        Ok(i) => i,
    };
    let token = match create_token_for_developer(id, &mut transaction).await {
        Err(e) => {
            transaction
                .rollback()
                .await
                .or(Err(ApiError::TransactionError))?;
            log::error!("{}", e);
            return Err(ApiError::InternalError);
        }
        Ok(t) => t,
    };
    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: token.to_string(),
    }))
}
