use crate::database::DatabaseError;
use chrono::{Days, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

/// Assumes developer ID exists
#[tracing::instrument(skip_all, fields(developer_id = %developer_id))]
pub async fn generate_token(
    developer_id: i32,
    with_expiry: bool,
    conn: &mut PgConnection,
) -> Result<Uuid, DatabaseError> {
    let token = Uuid::new_v4();
    let hash = sha256::digest(token.to_string());
    let expiry = {
        if with_expiry {
            Some(Utc::now().checked_add_days(Days::new(1)).unwrap())
        } else {
            None
        }
    };

    sqlx::query!(
        "INSERT INTO auth_tokens(token, developer_id, expires_at)
        VALUES ($1, $2, $3)",
        hash,
        developer_id,
        expiry
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(token)
}

#[tracing::instrument(skip_all)]
pub async fn remove_token(token: Uuid, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    let hash = sha256::digest(token.to_string());

    sqlx::query!(
        "DELETE FROM auth_tokens
        WHERE token = $1",
        hash
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}

#[tracing::instrument(skip_all, fields(developer_id = %developer_id))]
pub async fn remove_developer_tokens(
    developer_id: i32,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM auth_tokens
        WHERE developer_id = $1",
        developer_id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn cleanup(conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM auth_tokens
        WHERE expires_at < NOW()"
    )
    .execute(conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}
