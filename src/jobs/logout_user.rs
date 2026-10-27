use crate::database::repository::{auth_tokens, developers, refresh_tokens};
use sqlx::PgConnection;

pub async fn logout_user(username: &str, conn: &mut PgConnection) -> anyhow::Result<()> {
    let dev = developers::get_one_by_username(username, conn)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Developer {username} not found"))?;

    auth_tokens::remove_developer_tokens(dev.id, conn).await?;
    refresh_tokens::remove_developer_tokens(dev.id, conn).await?;

    Ok(())
}
