use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{database::DatabaseError, email::blocklist::ApprovedEmailAddress};

pub struct EmailChangeRequestRow {
    pub developer_id: i32,
    pub token: Uuid,
    pub new_email: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn find_one(
    id: i32,
    conn: &mut PgConnection,
) -> Result<Option<EmailChangeRequestRow>, DatabaseError> {
    sqlx::query_as!(
        EmailChangeRequestRow,
        "SELECT
            developer_id, token, new_email,
            created_at, expires_at
        FROM email_change_requests
        WHERE developer_id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{}", e))
    .map_err(|e| e.into())
}

pub async fn create(
    id: i32,
    token: &Uuid,
    new_email: &ApprovedEmailAddress,
    expires_at: &DateTime<Utc>,
    conn: &mut PgConnection,
) -> Result<EmailChangeRequestRow, DatabaseError> {
    sqlx::query_as!(
        EmailChangeRequestRow,
        "INSERT INTO email_change_requests
        (developer_id, token, new_email, expires_at)
        VALUES ($1, $2, $3, $4)
        RETURNING
            developer_id, token, new_email,
            created_at, expires_at",
        id,
        token,
        new_email.email().to_string(),
        expires_at
    )
    .fetch_one(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{}", e))
    .map_err(|e| e.into())
}

pub async fn delete(id: i32, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query_as!(
        EmailChangeRequestRow,
        "DELETE FROM email_change_requests
        WHERE developer_id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{}", e))
    .map(|_| ())
    .map_err(|e| e.into())
}
