use actix_web::{error::QueryPayloadError, http::header::ContentType, HttpRequest, HttpResponse};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Serialize, Deserialize)]
pub struct PaginatedData<T> {
    pub data: Vec<T>,
    pub count: i64,
}

#[derive(Debug, PartialEq)]
pub enum ApiError {
    FilesystemError,
    DbAcquireError,
    DbError,
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

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilesystemError => write!(f, "Unknown filesystem error"),
            Self::DbAcquireError => write!(f, "Database is busy"),
            Self::DbError => write!(f, "Unknown database error"),
            Self::BadRequest(message) => write!(f, "{}", message),
            Self::NotFound(message) => write!(f, "{}", message),
            Self::InternalError => write!(f, "Internal server error"),
            Self::Forbidden => write!(f, "You cannot perform this action"),
            Self::Unauthorized => write!(f, "You need to be authenticated to perform this action"),
        }
    }
}

impl actix_web::ResponseError for ApiError {
    fn error_response(&self) -> actix_web::HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code())
            .append_header(ContentType::json())
            .json(ApiResponse {
                error: self.to_string(),
                payload: "".to_string(),
            })
    }
    fn status_code(&self) -> StatusCode {
        match self {
            Self::FilesystemError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DbAcquireError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DbError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
        }
    }
}

pub fn query_error_handler(err: QueryPayloadError, _req: &HttpRequest) -> actix_web::Error {
    ApiError::BadRequest(err.to_string()).into()
}
