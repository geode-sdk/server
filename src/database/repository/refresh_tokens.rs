use crate::types::api::ApiError;
use chrono::{Days, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn generate_token(developer_id: i32, conn: &mut PgConnection) -> Result<Uuid, ApiError> {
    let token = Uuid::new_v4();
    let hash = sha256::digest(token.to_string());
    let expiry = Utc::now().checked_add_days(Days::new(30)).unwrap();

    sqlx::query!(
        "INSERT INTO refresh_tokens (token, developer_id, expires_at)
        VALUES ($1, $2, $3)",
        hash,
        developer_id,
        expiry
    )
    .execute(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to insert refresh token: {}", e);
        ApiError::DbError
    })?;

    Ok(token)
}

pub async fn remove_token(token: Uuid, conn: &mut PgConnection) -> Result<(), ApiError> {
    let hash = sha256::digest(token.to_string());
    sqlx::query!(
        "DELETE FROM refresh_tokens
        WHERE token = $1",
        hash
    )
    .execute(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to remove refresh token: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}

pub async fn remove_developer_tokens(
    developer_id: i32,
    conn: &mut PgConnection,
) -> Result<(), ApiError> {
    sqlx::query!(
        "DELETE FROM refresh_tokens
        WHERE developer_id = $1",
        developer_id
    )
    .execute(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to remove refresh tokens: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}

pub async fn cleanup(conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!(
        "DELETE FROM refresh_tokens
        WHERE expires_at < NOW()"
    )
    .execute(conn)
    .await
    .map_err(|e| {
        log::error!("Refresh token cleanup failed: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}
