use crate::database::repository::mod_downloads;
use crate::endpoints::ApiError;
use sqlx::PgConnection;

pub async fn cleanup_downloads(conn: &mut PgConnection) -> Result<(), ApiError> {
    mod_downloads::cleanup(conn).await?;

    Ok(())
}

