use actix_web::{http::StatusCode, HttpResponse};
use crate::types::{api::ApiResponse, models::mod_gd_version::PlatformParseError};

pub mod auth;
pub mod developers;
pub mod health;
pub mod loader;
pub mod mod_versions;
pub mod mods;
pub mod stats;
pub mod tags;

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("Authentication error: {0}")]
    Authentication(#[from] crate::auth::AuthenticationError),
    #[error("You do not have acces to this resource")]
    Authorization,
    #[error("{0}")]
    Database(#[from] crate::database::DatabaseError),
    #[error("Unknown database error")]
    SqlxError(#[from] sqlx::Error),
    #[error("Failed to parse response data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    TooManyRequests(String),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("{0}")]
    NotFound(String),
    #[error("Error: {0}")]
    PlatformParseError(#[from] PlatformParseError),
    #[error("Unable to unzip archive")]
    Zip(#[from] zip::result::ZipError),
    #[error("Failed to contact external resource: {0}")]
    Reqwest(#[from] reqwest::Error),
}

impl ApiError {
    pub fn as_response(&self) -> ApiResponse<String> {
        ApiResponse {
            error: self.to_string(),
            payload: "".into(),
        }
    }
}

impl actix_web::ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Authentication(..) => StatusCode::UNAUTHORIZED,
            ApiError::Authorization => StatusCode::FORBIDDEN,
            ApiError::Json(..) => StatusCode::BAD_REQUEST,
            ApiError::TooManyRequests(..) => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code()).json(self.as_response())
    }
}
