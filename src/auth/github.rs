use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT},
    Client, StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::{types::ipnetwork::IpNetwork, PgConnection};
use uuid::Uuid;

use crate::types::{api::ApiError, models::github_login_attempt::GithubLoginAttempt};

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

impl GithubClient {
    pub fn new(client_id: String, client_secret: String) -> GithubClient {
        GithubClient {
            client_id,
            client_secret,
        }
    }

    pub async fn start_auth(
        &self,
        ip: IpNetwork,
        pool: &mut PgConnection,
    ) -> Result<GithubLoginAttempt, ApiError> {
        #[derive(Serialize)]
        struct GithubStartAuthBody {
            client_id: String,
        }
        let found_request = GithubLoginAttempt::get_one_by_ip(ip, &mut *pool).await?;
        if let Some(r) = found_request {
            if r.is_expired() {
                let uuid = Uuid::parse_str(&r.uuid).unwrap();
                GithubLoginAttempt::remove(uuid, &mut *pool).await?;
            } else {
                return Ok(GithubLoginAttempt {
                    uuid: r.uuid.to_string(),
                    interval: r.interval,
                    uri: r.uri,
                    code: r.user_code,
                });
            }
        }
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        let client = match Client::builder().default_headers(headers).build() {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            }
            Ok(c) => c,
        };
        let body = GithubStartAuthBody {
            client_id: String::from(&self.client_id),
        };
        let json = match serde_json::to_string(&body) {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            }
            Ok(j) => j,
        };
        let result = match client
            .post("https://github.com/login/device/code")
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .body(json)
            .send()
            .await
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            }
            Ok(r) => r,
        };

        if result.status() != StatusCode::OK {
            log::error!("Couldn't connect to GitHub");
            return Err(ApiError::InternalError);
        }
        let body = match result.json::<GithubStartAuth>().await {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            }
            Ok(b) => b,
        };
        let uuid = GithubLoginAttempt::create(
            ip,
            body.device_code,
            body.interval,
            body.expires_in,
            &body.verification_uri,
            &body.user_code,
            &mut *pool,
        )
        .await?;

        Ok(GithubLoginAttempt {
            uuid: uuid.to_string(),
            interval: body.interval,
            uri: body.verification_uri,
            code: body.user_code,
        })
    }

    pub async fn poll_github(&self, device_code: &str) -> Result<String, ApiError> {
        #[derive(Serialize, Debug)]
        struct GithubPollAuthBody {
            client_id: String,
            device_code: String,
            grant_type: String,
        }
        let body = GithubPollAuthBody {
            client_id: String::from(&self.client_id),
            device_code: String::from(device_code),
            grant_type: String::from("urn:ietf:params:oauth:grant-type:device_code"),
        };
        let json = match serde_json::to_string(&body) {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::InternalError);
            }
            Ok(j) => j,
        };
        let client = Client::new();
        let resp = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .header(
                "Content-Type",
                HeaderValue::from_str("application/json").unwrap(),
            )
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .body(json)
            .send()
            .await;
        if resp.is_err() {
            log::info!("{}", resp.err().unwrap());
            return Err(ApiError::InternalError);
        }
        let resp = resp.unwrap();
        let body = resp.json::<serde_json::Value>().await.unwrap();
        match body.get("access_token") {
            None => {
                log::error!("{:?}", body);
                Err(ApiError::BadRequest(
                    "Request not accepted by user".to_string(),
                ))
            }
            Some(t) => Ok(String::from(t.as_str().unwrap())),
        }
    }

    pub async fn get_user(&self, token: String) -> Result<serde_json::Value, ApiError> {
        let client = Client::new();
        let resp = match client
            .get("https://api.github.com/user")
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

        Ok(body)
    }
}
