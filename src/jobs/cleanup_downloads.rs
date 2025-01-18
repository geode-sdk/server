use sqlx::PgConnection;
use crate::database::repository::mod_downloads;
use crate::types::api::ApiError;

pub async fn cleanup_downloads(conn: &mut PgConnection) -> Result<(), ApiError> {
    mod_downloads::cleanup(conn).await?;

    Ok(())
}