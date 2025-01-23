use actix_web::http::StatusCode;
use actix_web::{dev::ConnectionInfo, post, web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::{types::ipnetwork::IpNetwork, Acquire};
use uuid::Uuid;

use crate::config::AppData;
use crate::database::repository::{
    auth_tokens, developers, github_login_attempts, github_web_logins, refresh_tokens,
};
use crate::endpoints::auth::TokensResponse;
use crate::{
    auth::github,
    types::api::{ApiError, ApiResponse},
};

#[derive(Deserialize)]
struct PollParams {
    uuid: String,
    expiry: Option<bool>,
}

#[derive(Deserialize)]
struct TokenLoginParams {
    token: String,
}

#[derive(Deserialize)]
struct CallbackParams {
    code: String,
    state: String,
}

#[post("v1/login/github")]
pub async fn start_github_login(
    data: web::Data<AppData>,
    info: ConnectionInfo,
) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    let client = github::GithubClient::new(
        data.github().client_id().to_string(),
        data.github().client_secret().to_string(),
    );
    let ip = match info.realip_remote_addr() {
        None => return Err(ApiError::InternalError),
        Some(i) => i,
    };
    let net: IpNetwork = ip.parse().or(Err(ApiError::InternalError))?;

    let result = client.start_polling_auth(net, &mut pool).await?;
    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: result,
    }))
}

#[post("v1/login/github/web")]
pub async fn start_github_web_login(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let secret = github_web_logins::create_unique(&mut pool).await?;

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: format!(
            "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user&state={}",
            data.github().client_id(),
            format!("{}/login/github/callback", data.front_url()),
            secret.to_string()
        ),
    }))
}

#[post("v1/login/github/callback")]
pub async fn github_web_callback(
    json: web::Json<CallbackParams>,
    data: web::Data<AppData>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let parsed =
        Uuid::parse_str(&json.state).or(Err(ApiError::BadRequest("Invalid secret".into())))?;

    if !github_web_logins::exists(parsed, &mut pool).await? {
        return Err(ApiError::NotFound("Invalid secret".into()));
    }

    github_web_logins::remove(parsed, &mut pool).await?;

    let client = github::GithubClient::new(
        data.github().client_id().to_string(),
        data.github().client_secret().to_string(),
    );

    let token = client
        .poll_github(&json.code, false, Some(data.front_url()))
        .await?;

    let user = client
        .get_user(&token)
        .await
        .map_err(|_| ApiError::InternalError)?;

    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let developer = developers::fetch_or_insert_github(user.id, &user.username, &mut tx).await?;

    let token = auth_tokens::generate_token(developer.id, true, &mut tx).await?;
    let refresh = refresh_tokens::generate_token(developer.id, &mut tx).await?;

    tx.commit().await.or(Err(ApiError::TransactionError))?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: TokensResponse {
            access_token: token.to_string(),
            refresh_token: refresh.to_string(),
        },
    }))
}

#[post("v1/login/github/poll")]
pub async fn poll_github_login(
    json: web::Json<PollParams>,
    data: web::Data<AppData>,
    connection_info: ConnectionInfo,
) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;

    let uuid = Uuid::parse_str(&json.uuid).or(Err(ApiError::BadRequest("Invalid uuid".into())))?;

    let attempt = github_login_attempts::get_one_by_uuid(uuid, &mut pool)
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
        github_login_attempts::remove(uuid, &mut pool).await?;
        return Err(ApiError::BadRequest("Login attempt expired".to_string()));
    }

    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let client = github::GithubClient::new(
        data.github().client_id().to_string(),
        data.github().client_secret().to_string(),
    );
    github_login_attempts::poll_now(uuid, &mut tx).await?;
    let token = client.poll_github(&attempt.device_code, true, None).await?;
    github_login_attempts::remove(uuid, &mut tx).await?;

    // Create a new transaction after this point, because we need to commit the removal of the login attempt
    // It would be invalid for GitHub anyway

    tx.commit().await.or(Err(ApiError::TransactionError))?;

    let user = client
        .get_user(&token)
        .await
        .map_err(|_| ApiError::InternalError)?;

    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let developer = developers::fetch_or_insert_github(user.id, &user.username, &mut tx).await?;

    let expiry = json.expiry.is_some_and(|e| e);

    let token = auth_tokens::generate_token(developer.id, expiry, &mut tx).await?;
    let refresh = {
        if expiry {
            Some(refresh_tokens::generate_token(developer.id, &mut tx).await?)
        } else {
            None
        }
    };

    tx.commit().await.or(Err(ApiError::TransactionError))?;

    if expiry {
        Ok(HttpResponse::build(StatusCode::OK).json(ApiResponse {
            error: "".to_string(),
            payload: TokensResponse {
                access_token: token.to_string(),
                refresh_token: refresh.unwrap().to_string(),
            },
        }))
    } else {
        Ok(HttpResponse::build(StatusCode::OK).json(ApiResponse {
            error: "".to_string(),
            payload: token.to_string(),
        }))
    }
}

#[post("v1/login/github/token")]
pub async fn github_token_login(
    json: web::Json<TokenLoginParams>,
    data: web::Data<AppData>,
) -> Result<impl Responder, ApiError> {
    let client = github::GithubClient::new(
        data.github().client_id().to_string(),
        data.github().client_secret().to_string(),
    );

    let user = match client.get_user(&json.token).await {
        Err(_) => client
            .get_installation(&json.token)
            .await
            .map_err(|_| ApiError::BadRequest(format!("Invalid access token: {}", json.token)))?,

        Ok(u) => u,
    };

    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let developer = developers::fetch_or_insert_github(user.id, &user.username, &mut tx).await?;
    let token = auth_tokens::generate_token(developer.id, true, &mut tx).await?;

    tx.commit().await.or(Err(ApiError::TransactionError))?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: token.to_string(),
    }))
}
