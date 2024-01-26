use reqwest::{header::{HeaderMap, HeaderValue}, Client, RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize};

use crate::types::api::ApiError;

#[derive(Deserialize, Serialize)]
pub struct GithubStartAuth {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in: i32,
    interval: i32
}

#[derive(Serialize)]
pub struct GithubStartAuthBody {
    pub client_id: String
}


pub async fn start_auth(client_id: &str) -> Result<GithubStartAuth, ApiError> {
    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    let client = Client::builder()
        .default_headers(headers)
        .build();
    if client.is_err() {
        log::error!("{}", client.err().unwrap());
        return Err(ApiError::InternalError);
    }
    let client = client.unwrap();
    let body = GithubStartAuthBody {client_id: String::from(client_id)};
    let json = match serde_json::to_string(&body) {
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::InternalError);
        },
        Ok(j) => j
    };
    let result = client.execute(
        client.post("https://github.com/login/device/code")
            .body(json)
            .build().or(Err(ApiError::InternalError))?
    ).await;
    if result.is_err() {
        log::error!("{}", result.err().unwrap());
        return Err(ApiError::InternalError);
    }

    let result = result.unwrap();
    if result.status() != StatusCode::OK {
        log::error!("Couldn't connect to GitHub");
        return Err(ApiError::InternalError);
    }
    let body = result.json::<GithubStartAuth>().await.or(Err(ApiError::InternalError))?;

    Ok(body)
}

#[derive(Serialize)]
pub struct GithubPollAuthBody {
    client_id: String,
    device_code: String,
    grant_type: String
}

pub async fn poll_github(client_id: &str, device_code: &str) -> Result<serde_json::Value, ApiError> {
    let body = GithubPollAuthBody {
        client_id: String::from(client_id),
        device_code: String::from(device_code),
        grant_type: String::from("urn:ietf:params:oauth:grant-type:device_code")
    };
    let json = match serde_json::to_string(&body) {
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::InternalError);
        },
        Ok(j) => j
    };
    let client = Client::new();
    let resp = client.post("https://github.com/login/oauth/access_token")
        .header("Accept", HeaderValue::from_str("application/json").unwrap())
        .body(json)
        .send()
        .await;
    if resp.is_err() {
        log::info!("{}", resp.err().unwrap());
        return Err(ApiError::InternalError);
    }
    let resp = resp.unwrap();
    let status = resp.status();
    let body = resp.json::<serde_json::Value>().await.unwrap();

    if status != StatusCode::OK {
        log::info!("{:?}", body);
    }

    Ok(body)
}