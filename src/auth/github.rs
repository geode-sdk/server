use crate::database::repository::github_login_attempts;
use crate::types::models::github_login_attempt::StoredLoginAttempt;
use crate::types::api::ApiError;
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
                "GitHub OAuth device flow start request failed with code {}",
                res.status()
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

        Ok(github_login_attempts::create(
            ip,
            body.device_code,
            body.interval,
            body.expires_in,
            &body.verification_uri,
            &body.user_code,
            &mut *pool,
        )
        .await?)
    }

    pub async fn poll_github(&self, device_code: &str) -> Result<String, ApiError> {
        let resp = Client::new()
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .header(
                "Content-Type",
                HeaderValue::from_str("application/json").unwrap(),
            )
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .json(&json!({
                "client_id": &self.client_id,
                "device_code": device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
            }))
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

        Ok(resp.json::<GitHubFetchedUser>().await.map_err(|e| {
            log::error!("Failed to create GitHubFetchedUser: {}", e);
            ApiError::InternalError
        })?)
    }
}
