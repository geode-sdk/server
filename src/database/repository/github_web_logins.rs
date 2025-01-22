use crate::types::api::ApiError;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn create_unique(conn: &mut PgConnection) -> Result<Uuid, ApiError> {
    let unique = Uuid::new_v4();

    sqlx::query!("INSERT INTO github_web_logins (state) VALUES ($1)", unique)
        .execute(conn)
        .await
        .map_err(|e| {
            log::error!("Failed to create GitHub web login secret: {}", e);
            ApiError::DbError
        })?;

    Ok(unique)
}

pub async fn exists(uuid: Uuid, conn: &mut PgConnection) -> Result<bool, ApiError> {
    Ok(
        sqlx::query!("SELECT state FROM github_web_logins WHERE state = $1", uuid)
            .fetch_optional(conn)
            .await
            .map_err(|e| {
                log::error!("Failed to delete GitHub web login secret: {}", e);
                ApiError::DbError
            })?
            .is_some(),
    )
}

pub async fn remove(uuid: Uuid, conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!("DELETE FROM github_web_logins WHERE state = $1", uuid)
        .execute(conn)
        .await
        .map_err(|e| {
            log::error!("Failed to delete GitHub web login secret: {}", e);
            ApiError::DbError
        })?;

    Ok(())
}
