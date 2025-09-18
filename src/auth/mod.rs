pub mod github;

#[derive(thiserror::Error, Debug)]
pub enum AuthenticationError {
    #[error("No authentication token provided")]
    NoToken,
    #[error("Provided token is invalid")]
    InvalidToken,
    #[error("Failed to communicate with GitHub")]
    RequestError(#[from] reqwest::Error),
    #[error("{0}")]
    InternalError(String),
    #[error("{0}")]
    Database(#[from] crate::database::DatabaseError),
    #[error("Unknown database error")]
    SqlxError(#[from] sqlx::Error),
}
