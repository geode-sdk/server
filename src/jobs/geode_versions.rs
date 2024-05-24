use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use sqlx::PgConnection;

use crate::AppData;

pub async fn check_for_version(pool: &mut PgConnection) -> Result<(), String> {
    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    headers.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static("2022-11-28"),
    );
    let client = Client::builder().default_headers(headers);
    Ok(())
}
