use crate::database::DatabaseError;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn create_unique(conn: &mut PgConnection) -> Result<Uuid, DatabaseError> {
    let unique = Uuid::new_v4();

    sqlx::query!("INSERT INTO github_web_logins (state) VALUES ($1)", unique)
        .execute(conn)
        .await
        .inspect_err(|e| log::error!("Failed to create GitHub web login secret: {e}"))?;

    Ok(unique)
}

pub async fn exists(uuid: Uuid, conn: &mut PgConnection) -> Result<bool, DatabaseError> {
    sqlx::query!("SELECT state FROM github_web_logins WHERE state = $1", uuid)
        .fetch_optional(conn)
        .await
        .inspect_err(|e| log::error!("Failed to delete GitHub web login secret: {e}"))
        .map(|x| x.is_some())
        .map_err(|e| e.into())
}

pub async fn remove(uuid: Uuid, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!("DELETE FROM github_web_logins WHERE state = $1", uuid)
        .execute(conn)
        .await
        .inspect_err(|e| log::error!("Failed to delete GitHub web login secret: {e}"))?;

    Ok(())
}
