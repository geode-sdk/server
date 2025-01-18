use sqlx::PgConnection;
use uuid::Uuid;

use crate::types::api::ApiError;

pub async fn create_token_for_developer(
    id: i32,
    pool: &mut PgConnection,
) -> Result<Uuid, ApiError> {
    let token = Uuid::new_v4();
    let hash = sha256::digest(token.to_string());

    if let Err(e) = sqlx::query!(
        "INSERT INTO auth_tokens (developer_id, token) VALUES ($1, $2)",
        id,
        hash
    )
    .execute(&mut *pool)
    .await
    {
        log::error!("{}", e);
        return Err(ApiError::DbError);
    };

    Ok(token)
}

pub async fn invalidate_tokens_for_developer(
    id: i32,
    pool: &mut PgConnection,
) -> Result<(), ApiError> {
    if let Err(e) = sqlx::query!("DELETE FROM auth_tokens WHERE developer_id = $1", id)
        .execute(&mut *pool)
        .await
    {
        log::error!("{}", e);
        return Err(ApiError::DbError);
    };
    Ok(())
}

pub async fn invalidate_token_for_developer(
    id: i32,
    token: String,
    pool: &mut PgConnection,
) -> Result<(), ApiError> {
    let hash = sha256::digest(token);
    let result = match sqlx::query!(
        "DELETE FROM auth_tokens WHERE developer_id = $1 AND token = $2",
        id,
        hash
    )
    .execute(&mut *pool)
    .await
    {
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(r) => r,
    };

    if result.rows_affected() == 0 {
        log::error!("Couldn't delete token for developer {}", id);
        return Err(ApiError::InternalError);
    }
    Ok(())
}
