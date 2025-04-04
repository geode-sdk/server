use crate::database::repository::{auth_tokens, developers, refresh_tokens};
use crate::types::api::ApiError;
use sqlx::PgConnection;

pub async fn logout_user(username: &str, conn: &mut PgConnection) -> Result<(), ApiError> {
    let dev = developers::get_one_by_username(username, conn)
        .await?
        .ok_or(ApiError::NotFound("Developer not found".into()))?;

    auth_tokens::remove_developer_tokens(dev.id, conn).await?;
    refresh_tokens::remove_developer_tokens(dev.id, conn).await?;

    Ok(())
}
