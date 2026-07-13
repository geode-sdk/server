use crate::database::DatabaseError;
use sqlx::PgConnection;
use uuid::Uuid;

#[tracing::instrument(skip_all)]
pub async fn create_unique(conn: &mut PgConnection) -> Result<Uuid, DatabaseError> {
    let unique = Uuid::new_v4();

    sqlx::query!("INSERT INTO github_web_logins (state) VALUES ($1)", unique)
        .execute(conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(unique)
}

#[tracing::instrument(skip_all)]
pub async fn exists(uuid: Uuid, conn: &mut PgConnection) -> Result<bool, DatabaseError> {
    sqlx::query!("SELECT state FROM github_web_logins WHERE state = $1", uuid)
        .fetch_optional(conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))
        .map(|x| x.is_some())
        .map_err(|e| e.into())
}

#[tracing::instrument(skip_all)]
pub async fn remove(uuid: Uuid, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!("DELETE FROM github_web_logins WHERE state = $1", uuid)
        .execute(conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}
