use crate::database::repository::{auth_tokens, refresh_tokens};
use sqlx::PgConnection;

pub async fn token_cleanup(conn: &mut PgConnection) -> anyhow::Result<()> {
    auth_tokens::cleanup(conn).await?;
    refresh_tokens::cleanup(conn).await?;

    Ok(())
}
