use actix_web::{HttpResponse, http::header::ContentType};
use serde::{Serialize, Deserialize};
use std::fmt::Display;
use reqwest::StatusCode;

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
    BadRequest(String),
    NotFound(String)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub message: String,
    pub payload: T
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilesystemError => write!(f, "Unknown filesystem error"),
            Self::DbAcquireError => write!(f, "Database is busy"),
            Self::DbError => write!(f, "Unknown database error"),
            Self::BadRequest(message) => write!(f, "{}", message),
            Self::NotFound(message) => write!(f, "{}", message)
        }
    }
}

impl actix_web::ResponseError for ApiError {
    fn error_response(&self) -> actix_web::HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code())
            .append_header(ContentType::json())
            .json(ApiResponse {message: self.to_string(), payload: "".to_string()})
    }
    fn status_code(&self) -> StatusCode {
        match self {
            Self::FilesystemError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DbAcquireError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DbError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND
        }
    }
}