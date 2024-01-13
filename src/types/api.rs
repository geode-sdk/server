use actix_web::{HttpResponse, web, http::header::ContentType};
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

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilesystemError => write!(f, "filesystem error"),
            Self::DbAcquireError => write!(f, "database busy"),
            Self::DbError => write!(f, "database error"),
            Self::BadRequest(message) => write!(f, "{}", message),
            Self::NotFound(message) => write!(f, "{}", message)
        }
    }
}

impl actix_web::ResponseError for ApiError {
    fn error_response(&self) -> actix_web::HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code())
            .append_header(ContentType::json())
            .body(web::Json(self.to_string()).to_string())
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