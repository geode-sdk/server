use crate::database::repository::{auth_tokens, refresh_tokens};
use crate::endpoints::ApiError;
use sqlx::PgConnection;

pub async fn token_cleanup(conn: &mut PgConnection) -> Result<(), ApiError> {
    auth_tokens::cleanup(conn).await?;
    refresh_tokens::cleanup(conn).await?;

    Ok(())
}
