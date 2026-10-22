use crate::database::repository::mod_downloads;
use sqlx::PgConnection;

pub async fn cleanup_downloads(conn: &mut PgConnection) -> anyhow::Result<()> {
    mod_downloads::cleanup(conn).await?;

    Ok(())
}
