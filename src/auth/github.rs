use crate::database::repository::github_login_attempts;
use crate::types::api::ApiError;
use crate::types::models::github_login_attempt::StoredLoginAttempt;
use reqwest::{header::HeaderValue, Client};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{types::ipnetwork::IpNetwork, PgConnection};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct GithubStartAuth {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: i32,
    interval: i32,
}

pub struct GithubClient {
    client_id: String,
    client_secret: String,
}

#[derive(Serialize)]
pub struct GitHubDevicePollPayload {
    client_id: String,
    device_code: String,
    grant_type: String,
}

#[derive(Serialize)]
pub struct GitHubWebPollPayload {
    client_id: String,
    client_secret: String,
    code: String,
    redirect_uri: String,
}

#[derive(Deserialize)]
pub struct GitHubFetchedUser {
    pub id: i64,
    #[serde(alias = "login")]
    pub username: String,
}

impl GithubClient {
    pub fn new(client_id: String, client_secret: String) -> GithubClient {
        GithubClient {
            client_id,
            client_secret,
        }
    }

    pub async fn start_polling_auth(
        &self,
        ip: IpNetwork,
        pool: &mut PgConnection,
    ) -> Result<StoredLoginAttempt, ApiError> {
        if let Some(r) = github_login_attempts::get_one_by_ip(ip, pool).await? {
            if r.is_expired() {
                let uuid = Uuid::parse_str(&r.uuid).unwrap();
                github_login_attempts::remove(uuid, pool).await?;
            } else {
                return Ok(r);
            }
        }

        let res = Client::new()
            .post("https://github.com/login/device/code")
            .header("Accept", HeaderValue::from_static("application/json"))
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .json(&json!({
                "client_id": &self.client_id
            }))
            .send()
            .await
            .map_err(|e| {
                log::error!("Failed to start OAuth device flow with GitHub: {}", e);
                ApiError::InternalError
            })?;

        if !res.status().is_success() {
            log::error!(
                "GitHub OAuth device flow failed to start. Error code: {}. Body: {}",
                res.status(),
                res.text().await.unwrap_or("No body received".into())
            );
            return Err(ApiError::InternalError);
        }

        let body = res.json::<GithubStartAuth>().await.map_err(|e| {
            log::error!(
                "Failed to parse OAuth device flow response from GitHub: {}",
                e
            );
            ApiError::InternalError
        })?;

        github_login_attempts::create(
            ip,
            body.device_code,
            body.interval,
            body.expires_in,
            &body.verification_uri,
            &body.user_code,
            &mut *pool,
        )
        .await
    }

    pub async fn poll_github(
        &self,
        code: &str,
        is_device: bool,
        redirect_uri: Option<&str>,
    ) -> Result<String, ApiError> {
        let json = {
            if is_device {
                json!({
                    "client_id": &self.client_id,
                    "device_code": code,
                    "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
                })
            } else {
                let mut value = json!({
                    "client_id": &self.client_id,
                    "client_secret": &self.client_secret,
                    "code": code,
                });

                if let Some(r) = redirect_uri {
                    value["redirect_uri"] = json!(format!("{}/login/github/callback", r));
                }
                value
            }
        };

        let resp = Client::new()
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .header(
                "Content-Type",
                HeaderValue::from_str("application/json").unwrap(),
            )
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .json(&json)
            .send()
            .await
            .map_err(|e| {
                log::error!("Failed to poll GitHub for developer access token: {}", e);
                ApiError::InternalError
            })?;

        Ok(resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| {
                log::error!("Failed to decode GitHub response: {}", e);
                ApiError::InternalError
            })?
            .get("access_token")
            .ok_or(ApiError::BadRequest("Request not accepted by user".into()))?
            .as_str()
            .ok_or_else(|| {
                log::error!("Invalid access_token received from GitHub");
                ApiError::InternalError
            })?
            .to_string())
    }

    pub async fn get_user(&self, token: &str) -> Result<GitHubFetchedUser, ApiError> {
        let resp = Client::new()
            .get("https://api.github.com/user")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .header("User-Agent", "geode_index")
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| {
                log::error!("Request to https://api.github.com/user failed: {}", e);
                ApiError::InternalError
            })?;

        if !resp.status().is_success() {
            return Err(ApiError::InternalError);
        }

        resp.json::<GitHubFetchedUser>().await.map_err(|e| {
            log::error!("Failed to create GitHubFetchedUser: {}", e);
            ApiError::InternalError
        })
    }

    pub async fn get_installation(&self, token: &str) -> Result<GitHubFetchedUser, ApiError> {
        let client = Client::new();
        let resp = match client
            .get("https://api.github.com/installation/repositories")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .header("User-Agent", "geode_index")
            .bearer_auth(token)
            .send()
            .await
        {
            Err(e) => {
                log::info!("{}", e);
                return Err(ApiError::InternalError);
            }
            Ok(r) => r,
        };

        if !resp.status().is_success() {
            return Err(ApiError::InternalError);
        }

        let body = match resp.json::<serde_json::Value>().await {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            }
            Ok(b) => b,
        };

        let repos = match body.get("repositories").and_then(|r| r.as_array()) {
            None => {
                return Err(ApiError::InternalError);
            }
            Some(r) => r,
        };

        if repos.len() != 1 {
            return Err(ApiError::InternalError);
        }

        let owner = repos[0]
            .get("owner")
            .ok_or(ApiError::InternalError)?
            .clone();

        serde_json::from_value(owner).map_err(|e| {
            log::error!("Failed to create GitHubFetchedUser: {}", e);
            ApiError::InternalError
        })
    }
}
