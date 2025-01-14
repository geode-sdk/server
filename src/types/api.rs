use std::error::Error;
use actix_web::{error::QueryPayloadError, http::header::ContentType, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Write};
use actix_web::body::BoxBody;
use actix_web::http::StatusCode;

#[derive(Serialize, Deserialize)]
pub struct PaginatedData<T> {
    pub data: Vec<T>,
    pub count: i64,
}

#[derive(Debug, PartialEq)]
pub enum ApiError2 {
    InternalError(String),
    NotFound(String),
    Unauthorized(String),
    Forbidden(String)
}

impl Error for ApiError2 {}
impl Display for ApiError2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError2::InternalError(err) => write!(f, "Internal Server Error: {}", err),
            ApiError2::NotFound(err) => write!(f, "Not found: {}", err),
            ApiError2::Unauthorized(err) => write!(f, "Unauthorized: {}", err),
            ApiError2::Forbidden(err) => write!(f, "Forbidden: {}", err)
        }
    }
}

impl actix_web::ResponseError for ApiError2 {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError2::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError2::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError2::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError2::Forbidden(_) => StatusCode::FORBIDDEN
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::build(self.status_code())
            .append_header(ContentType::json())
            .json(ApiResponse {
                error: self.to_string(),
                payload: "".to_string(),
            })
    }
}

#[derive(Debug, PartialEq)]
pub enum ApiError {
    FilesystemError,
    DbAcquireError,
    DbError,
    TransactionError,
    InternalError,
    BadRequest(String),
    NotFound(String),
    Unauthorized,
    Forbidden,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub error: String,
    pub payload: T,
}

impl Error for ApiError {}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilesystemError => write!(f, "Unknown filesystem error"),
            Self::DbAcquireError => write!(f, "Database is busy"),
            Self::DbError => write!(f, "Unknown database error"),
            Self::TransactionError => write!(f, "Unknown transaction error"),
            Self::BadRequest(message) => write!(f, "{}", message),
            Self::NotFound(message) => write!(f, "{}", message),
            Self::InternalError => write!(f, "Internal server error"),
            Self::Forbidden => write!(f, "You cannot perform this action"),
            Self::Unauthorized => write!(f, "You need to be authenticated to perform this action"),
        }
    }
}

impl actix_web::ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::FilesystemError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DbAcquireError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DbError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::TransactionError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
        }
    }
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code())
            .append_header(ContentType::json())
            .json(ApiResponse {
                error: self.to_string(),
                payload: "".to_string(),
            })
    }
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
