pub mod repository;

#[derive(thiserror::Error, Debug)]
pub enum DatabaseError {
    #[error("invalid data in column")]
    InvalidData,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Unknown database error")]
    SqlxError(#[from] sqlx::Error),
}
