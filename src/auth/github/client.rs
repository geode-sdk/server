use reqwest::{header::HeaderMap, Client, StatusCode};
use serde::Deserialize;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    auth::oauth_client::{PollResult, PollingOAuthClient},
    repositories,
    types::{api::ApiError, models::oauth_attempt::OAuthAttempt},
};

pub struct GitHubOAuthClient {
    client_id: String,
    client_secret: String,
    reqwest: Client,
}

impl PollingOAuthClient for GitHubOAuthClient {
    fn new(client_id: &str, client_secret: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("Accept", "application/vnd.github+json".parse().unwrap());

        let client = Client::builder()
            .user_agent("geode_index")
            .default_headers(headers)
            .build()
            .unwrap();

        GitHubOAuthClient {
            client_id: String::from(client_id),
            client_secret: String::from(client_secret),
            reqwest: client,
        }
    }

    async fn start_auth(
        &self,
        callback_url: &str,
        connection: &mut sqlx::PgConnection,
    ) -> Result<(String, OAuthAttempt), ApiError> {
        let secret = Uuid::new_v4();

        let url: String = format!(
            "https://github.com/login/oauth/authorize?client_id={}&state={}&redirect_uri={}",
            self.client_id, secret, callback_url
        );

        Ok((
            url,
            repositories::oauth_attempts::create(secret, connection).await?,
        ))
    }

    async fn get_user_profile(&self, oauth_token: &str) -> Result<Option<(i64, String)>, ApiError> {
        #[derive(Deserialize)]
        struct UserProfile {
            id: i64,
            login: String,
        }

        #[derive(Deserialize)]
        struct ErrorType {
            error: String,
        }

        match self
            .reqwest
            .get("https://api.github.com/user")
            .bearer_auth(oauth_token)
            .send()
            .await
        {
            Ok(r) => {
                if !r.status().is_success() {
                    if r.status() == StatusCode::UNAUTHORIZED {
                        match r.json::<ErrorType>().await {
                            Err(_) => (),
                            Ok(v) => log::info!("{}", v.error),
                        };
                        return Ok(None);
                    }

                    return Err(ApiError::InternalError);
                }

                let body = match r.json::<UserProfile>().await {
                    Err(e) => {
                        log::error!("{}", e);
                        return Err(ApiError::InternalError);
                    }
                    Ok(v) => v,
                };

                Ok(Some((body.id, body.login)))
            }
            Err(e) => {
                log::error!("Failed to get user profile: {}", e);
                Err(ApiError::InternalError)
            }
        }
    }

    async fn exchange_tokens(&self, code: String) -> Result<Option<(String, String)>, ApiError> {
        #[derive(Deserialize)]
        struct TokenResult {
            access_token: String,
            refresh_token: String,
        }

        #[derive(Deserialize)]
        struct ErrorType {
            error: String,
        }

        match self
            .reqwest
            .post("https://github.com/login/oauth/access_token")
            .query(&[
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("code", &code),
            ])
            .send()
            .await
        {
            Ok(r) => {
                let status = r.status();
                if !status.is_success() {
                    if status == StatusCode::UNAUTHORIZED {
                        return Ok(None);
                    }

                    let error = r.json::<ErrorType>().await.unwrap_or(ErrorType {
                        error: "Unidentified".to_string(),
                    });

                    match error.error.as_str() {
                        "bad_verification_code" => {
                            return Err(ApiError::BadRequest("Code expired or invalid".to_string()))
                        },
                        "unverified_user_email" => {
                            return Err(ApiError::BadRequest("The email of the GitHub user isn't verified. Please verify your email.".to_string()))
                        }
                        _ => return Err(ApiError::InternalError),
                    }
                }

                let body = match r.json::<TokenResult>().await {
                    Err(e) => {
                        log::error!("{}", e);
                        return Err(ApiError::InternalError);
                    }
                    Ok(v) => v,
                };

                Ok(Some((body.access_token, body.refresh_token)))
            }
            Err(e) => {
                log::error!("Failed to exchange tokens: {}", e);
                Err(ApiError::InternalError)
            }
        }
    }

    async fn poll(
        &self,
        secret: Uuid,
        connection: &mut PgConnection,
    ) -> Result<PollResult, ApiError> {
        let found = repositories::oauth_attempts::find_one(&secret, connection).await?;
        if let Some(found) = found {
            if found.is_expired() {
                let _ = repositories::oauth_attempts::delete(found.uid, connection).await;
                return Ok(PollResult::Expired);
            }

            if found.too_fast() {
                return Ok(PollResult::TooFast);
            }

            if found.token.is_some() && found.refresh_token.is_some() {
                let _ = repositories::oauth_attempts::delete(found.uid, connection).await;
                Ok(PollResult::Done((
                    found.token.unwrap(),
                    found.refresh_token.unwrap(),
                )))
            } else {
                Ok(PollResult::Pending)
            }
        } else {
            Err(ApiError::NotFound(
                "No auth session found for UID".to_string(),
            ))
        }
    }
}
