use crate::types::api::ApiError;
use sqlx::PgConnection;

pub async fn increment_downloads(id: i32, conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE mod_versions
        SET download_count = download_count + 1
        WHERE id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!(
            "Failed to increment downloads for mod_version {}: {}",
            id,
            e
        );
        ApiError::DbError
    })?;

    Ok(())
}
