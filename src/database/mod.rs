pub mod repository;

#[derive(thiserror::Error, Debug)]
pub enum DatabaseError {
    #[error("Unknown database error")]
    SqlxError(#[from] sqlx::Error)
}
