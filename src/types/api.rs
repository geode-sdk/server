use actix_web::{error::QueryPayloadError, HttpRequest};
use serde::{Deserialize, Serialize};

use crate::endpoints::ApiError;

#[derive(Serialize, Deserialize)]
pub struct PaginatedData<T> {
    pub data: Vec<T>,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub error: String,
    pub payload: T,
}

pub fn query_error_handler(err: QueryPayloadError, _req: &HttpRequest) -> actix_web::Error {
    ApiError::BadRequest(err.to_string()).into()
}

pub fn create_download_link(app_url: &str, mod_id: &str, version: &str) -> String {
    format!(
        "{}/v1/mods/{}/versions/{}/download",
        app_url, mod_id, version
    )
}
