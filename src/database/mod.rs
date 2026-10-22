use std::ops::Deref;

use sqlx::{Acquire, migrate::Migrate};

pub mod repository;

#[derive(thiserror::Error, Debug)]
pub enum DatabaseError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Unknown database error")]
    SqlxError(#[from] sqlx::Error),
}

pub async fn migrate<'a, A>(conn: A) -> anyhow::Result<()>
where
    A: Acquire<'a>,
    <A::Connection as Deref>::Target: Migrate,
{
    sqlx::migrate!()
        .run(conn)
        .await
        .inspect_err(|e| tracing::error!("Failed to run migrations: {e}"))?;
    Ok(())
}
