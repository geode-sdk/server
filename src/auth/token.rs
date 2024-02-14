use sqlx::PgConnection;
use uuid::Uuid;

use crate::types::api::ApiError;

pub async fn create_token_for_developer(
    id: i32,
    pool: &mut PgConnection,
) -> Result<Uuid, ApiError> {
    let result = sqlx::query!(
        "INSERT INTO auth_tokens (developer_id) VALUES ($1) returning token",
        id
    )
    .fetch_one(&mut *pool)
    .await;
    let result = match result {
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(r) => r,
    };

    Ok(result.token)
}
