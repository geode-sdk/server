pub mod github;

#[derive(thiserror::Error, Debug)]
pub enum AuthenticationError {
    #[error("No authentication token provided")]
    NoToken,
    #[error("Provided token is invalid")]
    InvalidToken,
    #[error("Unknown database error")]
    SqlxError(#[from] sqlx::Error)
}
