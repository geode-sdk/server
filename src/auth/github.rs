use crate::auth::AuthenticationError;
use crate::database::repository::github_login_attempts;
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

#[derive(Deserialize)]
pub enum GithubDeviceFlowErrorString {
    #[serde(rename(deserialize = "authorization_pending"))]
    AuthorizationPending,
    #[serde(rename(deserialize = "slow_down"))]
    SlowDown,
    #[serde(rename(deserialize = "expired_token"))]
    ExpiredToken,
    #[serde(rename(deserialize = "unsupported_grant_type"))]
    UnsupportedGrantType,
    #[serde(rename(deserialize = "incorrect_client_credentials"))]
    IncorrectClientCredentials,
    #[serde(rename(deserialize = "incorrect_device_code"))]
    IncorrectDeviceCode,
    #[serde(rename(deserialize = "access_denied"))]
    AccessDenied,
    #[serde(rename(deserialize = "device_flow_disabled"))]
    DeviceFlowDisabled,
    Unknown,
}

impl Default for GithubDeviceFlowErrorString {
    fn default() -> Self {
        GithubDeviceFlowErrorString::Unknown
    }
}

#[derive(Deserialize)]
pub struct GithubErrorResponse {
    error: GithubDeviceFlowErrorString,
    error_description: String,
    error_uri: String,
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
    ) -> Result<StoredLoginAttempt, AuthenticationError> {
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
            .inspect_err(|e| log::error!("Failed to start OAuth device flow with GitHub: {e}"))?;

        if !res.status().is_success() {
            log::error!(
                "GitHub OAuth device flow failed to start. Error code: {}. Body: {}",
                res.status(),
                res.text().await.unwrap_or("No body received".into())
            );
            return Err(AuthenticationError::InternalError(
                "Failed to start GitHub device flow".into(),
            ));
        }

        let body = res
            .json::<GithubStartAuth>()
            .await
            .inspect_err(|e| {
                log::error!("Failed to parse OAuth device flow response from GitHub: {e}")
            })
            .or(Err(AuthenticationError::InternalError(
                "Failed to parse response from GitHub".into(),
            )))?;

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
        .map_err(|e| e.into())
    }

    pub async fn poll_github(
        &self,
        code: &str,
        is_device: bool,
        redirect_uri: Option<&str>,
    ) -> Result<String, AuthenticationError> {
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
            .inspect_err(|e| {
                log::error!("Failed to poll GitHub for developer access token: {e}")
            })?;

        Ok(resp
            .json::<serde_json::Value>()
            .await
            .inspect_err(|e| log::error!("Failed to decode GitHub response: {e}"))?
            .get("access_token")
            .ok_or(AuthenticationError::UserAuthPending)?
            .as_str()
            .ok_or_else(|| {
                log::error!("Invalid access_token received from GitHub");
                AuthenticationError::InternalError(
                    "Failed to retrieve access token from GitHub".into(),
                )
            })?
            .to_string())
    }
    pub async fn get_user(&self, token: &str) -> Result<GitHubFetchedUser, AuthenticationError> {
        let resp = Client::new()
            .get("https://api.github.com/user")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .header("User-Agent", "geode_index")
            .bearer_auth(token)
            .send()
            .await?;

        if !resp.status().is_success() {
            log::error!(
                "github::get_user: received non-2xx response: {}. Body: {}",
                resp.status(),
                resp.text().await.unwrap_or("No response body".into())
            );
            return Err(AuthenticationError::InternalError(
                "Failed to fetch user from GitHub API, received non 2xx response".into(),
            ));
        }

        resp.json::<GitHubFetchedUser>()
            .await
            .inspect_err(|e| log::error!("github::get_user: failed to parse response: {e}"))
            .or(Err(AuthenticationError::InternalError(
                "Failed to parse user JSON received from GitHub".into(),
            )))
    }

    pub async fn get_installation(
        &self,
        token: &str,
    ) -> Result<GitHubFetchedUser, AuthenticationError> {
        let client = Client::new();
        let resp = client
            .get("https://api.github.com/installation/repositories")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .header("User-Agent", "geode_index")
            .bearer_auth(token)
            .send()
            .await
            .inspect_err(|e| {
                log::error!("github::get_installation: failed to fetch repositories: {e}")
            })?;

        if !resp.status().is_success() {
            log::error!(
                "github::get_installation: received non-2xx response: {}. Body: {}",
                resp.status(),
                resp.text().await.unwrap_or("No response body".into())
            );
            return Err(AuthenticationError::InternalError(
                "Received non-2xx response from GitHub".into(),
            ));
        }

        let body = resp
            .json::<serde_json::Value>()
            .await
            .inspect_err(|e| log::error!("github::get_installation: failed to parse response: {e}"))
            .or(Err(AuthenticationError::InternalError(
                "Failed to parse response from GitHub".into(),
            )))?;

        let repos = body.get("repositories").and_then(|r| r.as_array()).ok_or(
            AuthenticationError::InternalError(
                "Failed to get repository array from GitHub response".into(),
            ),
        )?;

        if repos.len() != 1 {
            return Err(AuthenticationError::InternalError(
                "Failed to get repository from GitHub: array size isn't 1".into(),
            ));
        }

        let owner = repos[0]
            .get("owner")
            .ok_or(AuthenticationError::InternalError(
                "Didn't find owner key on repository".into(),
            ))?
            .clone();

        serde_json::from_value(owner)
            .inspect_err(|e| {
                log::error!(
                    "github::get_installation: failed to extract owner from serde_json value: {e}"
                )
            })
            .or(Err(AuthenticationError::InternalError(
                "Failed to get GitHub user from installation".into(),
            )))
    }
}
