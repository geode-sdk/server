use crate::auth::github::GitHubFetchedUser;
use crate::integration::github::client::GitHubClientBuilder;
use crate::types::api::ApiError;
use reqwest::header::HeaderValue;
use reqwest::{Client, Method};
use url::Url;

pub async fn get_one(
    token: &str,
    client_builder: &GitHubClientBuilder,
) -> Result<GitHubFetchedUser> {
    let response = client_builder
        .with_basic_auth(true)
        .get("/login/device/code")?
        .bearer_auth(token)
        .send()
        .await;

    let response = client_builder
        .base_url(Url::parse("https://api.github.com").unwrap())
        .with_basic_auth(false)
        .get("/user")?
        .bearer_auth(token)
        .send()
        .await;

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
}
