use actix_web::{get, post, web, HttpResponse, Responder};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use sqlx::Acquire;
use uuid::Uuid;

use crate::{
    auth::{
        github::client::GitHubOAuthClient,
        oauth_client::{PollResult, PollingOAuthClient},
    },
    repositories::{self, developer},
    types::{
        api::{ApiError, ApiResponse},
        models::developer::Developer,
    },
    AppData,
};

#[derive(Deserialize)]
struct PollParams {
    uuid: Uuid,
}

#[derive(Deserialize)]
struct CallbackParams {
    code: Option<String>,
    error: Option<String>,
    #[serde(default = "default_show_tokens")]
    show_tokens: bool,
    state: Uuid,
}

fn default_show_tokens() -> bool {
    false
}

#[post("v1/login/github")]
pub async fn start_github_login(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let client = GitHubOAuthClient::new(&data.github_client_id, &data.github_client_secret);

    let result = client
        .start_auth(
            &format!("{}/v1/login/github/callback", data.app_url),
            &mut pool,
        )
        .await?;
    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: json!({ "url": result.0, "attempt": result.1 }),
    }))
}

#[post("v1/login/github/poll")]
pub async fn poll_github_login(
    json: web::Json<PollParams>,
    data: web::Data<AppData>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let client = GitHubOAuthClient::new(&data.github_client_id, &data.github_client_secret);
    let result = client.poll(json.uuid, &mut transaction).await?;

    let resp_data = match result {
        PollResult::Done(_) => (StatusCode::OK, ""),
        PollResult::Expired => (StatusCode::GONE, "This authentication session has expired"),
        PollResult::Pending => (StatusCode::BAD_REQUEST, "The user hasn't authenticated yet"),
        PollResult::TooFast => (StatusCode::TOO_MANY_REQUESTS, "Slow down"),
    };

    let result = match result {
        PollResult::Pending | PollResult::Expired | PollResult::TooFast => {
            transaction
                .commit()
                .await
                .or(Err(ApiError::TransactionError))?;
            return Ok(HttpResponse::build(resp_data.0).json(ApiResponse {
                error: resp_data.1.to_string(),
                payload: "",
            }));
        }
        PollResult::Done(result) => result,
    };

    let user_info: Option<(i64, String)> = client.get_user_profile(&result.0).await?;

    if user_info.is_none() {
        let _ = transaction.rollback().await;
        return Err(ApiError::InternalError);
    }

    let user_info = user_info.unwrap();

    // Create a new transaction after this point, because we need to commit the removal of the login attempt
    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    if let Some(dev) = Developer::get_by_github_id(user_info.0, &mut pool).await? {
        return Ok(HttpResponse::build(resp_data.0).json(ApiResponse {
            error: "".to_string(),
            payload: json!({
                "token": result.0,
                "refresh_token": result.1,
                "user": Developer {
                        id: dev.id,
                        username: dev.username,
                        display_name: dev.display_name,
                        verified: dev.verified,
                        admin: dev.admin,
                        is_owner: None
                    }
            }),
        }));
    }

    let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let developer = repositories::developer::create(
        developer::CreateDto {
            github_id: user_info.0,
            username: user_info.1,
        },
        &mut transaction,
    )
    .await?;

    transaction
        .commit()
        .await
        .or(Err(ApiError::TransactionError))?;

    Ok(HttpResponse::build(resp_data.0).json(ApiResponse {
        error: "".to_string(),
        payload: json!({
            "token": result.0,
            "refresh_token": result.1,
            "user": developer
        }),
    }))
}

#[get("v1/login/github/callback")]
pub async fn oauth_callback(
    data: web::Data<AppData>,
    query: web::Query<CallbackParams>,
) -> Result<impl Responder, ApiError> {
    if query.code.as_ref().is_none() {
        if let Some(e) = &query.error {
            return Ok(HttpResponse::build(StatusCode::UNAUTHORIZED).body(e.clone()));
        }
    }

    let code = query.code.clone().unwrap();

    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let found = repositories::oauth_attempts::find_one(&query.state, &mut pool).await?;

    let client = GitHubOAuthClient::new(&data.github_client_id, &data.github_client_secret);
    match client.exchange_tokens(code).await? {
        None => Err(ApiError::Unauthorized),
        Some(tokens) => {
            if let Some(found) = found {
                let _ = repositories::oauth_attempts::set_tokens(
                    &found.uid, &tokens.0, &tokens.1, &mut pool,
                )
                .await?;
            }

            if query.show_tokens {
                Ok(HttpResponse::build(StatusCode::OK).json(ApiResponse {
                    error: "".to_string(),
                    payload: json!({
                        "token": tokens.0,
                        "refresh_token": tokens.1
                    }),
                }))
            } else {
                Ok(HttpResponse::build(StatusCode::OK).json(ApiResponse {
                    error: "".to_string(),
                    payload: "All done, you can return to the client!",
                }))
            }
        }
    }
}
