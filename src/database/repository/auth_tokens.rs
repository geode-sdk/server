use crate::types::api::ApiError;
use sqlx::PgConnection;
use uuid::Uuid;

/// Assumes developer ID exists
pub async fn generate_token(developer_id: i32, conn: &mut PgConnection) -> Result<Uuid, ApiError> {
    let token = Uuid::new_v4();
    let hash = sha256::digest(token.to_string());

    sqlx::query!(
        "INSERT INTO auth_tokens(token, developer_id)
        VALUES ($1, $2)",
        hash,
        developer_id
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!(
            "Failed to insert auth_token for developer {}: {}",
            developer_id,
            e
        );
        ApiError::DbError
    })?;

    Ok(token)
}

pub async fn remove_token(token: Uuid, conn: &mut PgConnection) -> Result<(), ApiError> {
    let hash = sha256::digest(token.to_string());

    sqlx::query!(
        "DELETE FROM auth_tokens
        WHERE token = $1",
        hash
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to remove auth token: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}

pub async fn remove_developer_tokens(
    developer_id: i32,
    conn: &mut PgConnection,
) -> Result<(), ApiError> {
    sqlx::query!(
        "DELETE FROM auth_tokens
        WHERE developer_id = $1",
        developer_id
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to wipe developer tokens: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}
