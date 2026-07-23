use crate::{
    mod_zip::ModZipError,
    types::{api::ApiResponse, models::mod_gd_version::PlatformParseError},
};
use actix_web::{HttpResponse, http::StatusCode};
use validator::{ValidationError, ValidationErrors};

pub mod auth;
pub mod deprecations;
pub mod developers;
pub mod health;
pub mod loader;
pub mod mod_status_badge;
pub mod mod_version_submissions;
pub mod mod_versions;
pub mod mods;
pub mod stats;
pub mod tags;

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("Authentication error: {0}")]
    Authentication(#[from] crate::auth::AuthenticationError),
    #[error("You do not have access to this resource")]
    Authorization,
    #[error("{0}")]
    Database(#[from] crate::database::DatabaseError),
    #[error("{0}")]
    Storage(#[from] crate::storage::StorageError),
    #[error("{0}")]
    ModZip(#[from] ModZipError),
    #[error("Database error")]
    SqlxError(#[from] sqlx::Error),
    #[error("Failed to parse response data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    #[allow(dead_code)]
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
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
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
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(..) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code()).json(self.as_response())
    }
}

// validator errors

impl From<ValidationError> for ApiError {
    fn from(value: ValidationError) -> Self {
        ApiError::BadRequest(format_validation_error(&value))
    }
}

impl From<ValidationErrors> for ApiError {
    fn from(value: ValidationErrors) -> Self {
        ApiError::BadRequest(format_validation_errors(&value))
    }
}

pub fn format_validation_error(e: &validator::ValidationError) -> String {
    if let Some(msg) = &e.message {
        return msg.to_string();
    }

    match e.code.as_ref() {
        "length" => {
            let min = e
                .params
                .get("min")
                .and_then(|v| v.as_u64())
                .map(|v| v.to_string());

            let max = e
                .params
                .get("max")
                .and_then(|v| v.as_u64())
                .map(|v| v.to_string());

            match (min, max) {
                (Some(min), Some(max)) => {
                    format!("length must be between {min} and {max} characters")
                }
                (Some(min), None) => format!("length must be at least {min} characters"),
                (None, Some(max)) => format!("length must be at most {max} characters"),
                (None, None) => "invalid length".to_string(),
            }
        }

        e => e.to_owned(),
    }
}

pub fn format_validation_errors(e: &validator::ValidationErrors) -> String {
    use validator::ValidationErrorsKind;

    let mut str_errors = Vec::new();

    for (field, err) in e.errors() {
        match err {
            ValidationErrorsKind::Struct(s) => {
                str_errors.push(format!(
                    "field '{field}' is invalid ({})",
                    format_validation_errors(s)
                ));
            }

            ValidationErrorsKind::Field(errors) => {
                let joined = errors
                    .iter()
                    .map(format_validation_error)
                    .collect::<Vec<_>>()
                    .join(", ");

                str_errors.push(format!("field '{field}' is invalid: {}", joined));
            }

            ValidationErrorsKind::List(map) => {
                for (index, errors) in map {
                    str_errors.push(format!(
                        "field '{field}' at index {index} is invalid ({})",
                        format_validation_errors(errors)
                    ));
                }
            }
        }
    }

    str_errors.join(", ")
}
