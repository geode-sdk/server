use actix_web::{dev::ConnectionInfo, post, web, Responder};
use serde::Deserialize;
use sqlx::{types::ipnetwork::IpNetwork, Acquire, PgConnection};
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

#[derive(Deserialize)]
struct TokenLoginParams {
    token: String,
}

async fn developer_from_token(
    pool: &mut PgConnection,
    user: serde_json::Value
) -> Result<Uuid, Option<ApiError>> {
    let id = user.get("id").ok_or(None)?.as_i64().unwrap();
    let username = user.get("login").ok_or(None)?;

    let dev_id = match Developer::get_by_github_id(id, pool).await? {
        Some(x) => x.id,
        None => Developer::create(id, username.to_string(), pool).await?
    };

    create_token_for_developer(dev_id, pool).await.map_err(Some)
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
        None => {
            log::error!("Couldn't parse IP from request");
            return Err(ApiError::InternalError);
        }
        Some(i) => i,
    };
    let net: IpNetwork = match ip.parse() {
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

    // Create a new transaction after this point, because we need to commit the removal of the login attempt

    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    let user = client.get_user(&token).await.map_err(|_| ApiError::InternalError)?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let token = match developer_from_token(&mut transaction, user).await {
        Ok(t) => t,
        Err(e) => {
            if let Some(e) = e {
                log::error!("{}", e);
            }

            transaction.rollback().await.map_or_else(
                |_| Err(ApiError::TransactionError),
                |_| Err(ApiError::InternalError)
            )?
        }
    };

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

    let user = match client.get_user(&json.token).await {
        Err(_) => client.get_installation(&json.token).await.map_err(|_|
            ApiError::BadRequest(format!("Invalid access token: {}", json.token))
        )?,

        Ok(u) => u
    };

    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let token = match developer_from_token(&mut transaction, user).await {
        Ok(t) => t,

        Err(e) => {
            if let Some(e) = e {
                log::error!("{}", e);
            }

            transaction.rollback().await.map_or_else(
                |_| Err(ApiError::TransactionError),
                |_| Err(ApiError::InternalError)
            )?
        }
    };

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: token.to_string()
    }))
}
