use sqlx::PgConnection;

pub async fn migrate(pool: &mut PgConnection) -> anyhow::Result<()> {
    if let Err(e) = sqlx::migrate!("./migrations").run(&mut *pool).await {
        tracing::error!("Error encountered while running migrations: {}", e);
    }

    Ok(())
}
