use sqlx::PgConnection;
use uuid::Uuid;

use crate::types::api::ApiError;

pub async fn create_token_for_developer(
    id: i32,
    pool: &mut PgConnection,
) -> Result<Uuid, ApiError> {
    let token = Uuid::new_v4();
    let hash = sha256::digest(token.to_string());

    let count = match sqlx::query_scalar!(
        "SELECT COUNT(*) FROM auth_tokens WHERE developer_id = $1",
        id
    )
    .fetch_one(&mut *pool)
    .await
    {
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(c) => c,
    };

    if count == Some(5) {
        return Err(ApiError::BadRequest(
            "You have reached the max amount of tokens (5). Invalidate your tokens or use your currently active ones.".to_string(),
        ));
    }

    if let Err(e) = sqlx::query!(
        "INSERT INTO auth_tokens (developer_id, token) VALUES ($1, $2)",
        id,
        hash
    )
    .fetch_one(&mut *pool)
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
