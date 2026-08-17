use argon2::PasswordHash;
use chrono::Utc;
use password_hash::PasswordHashString;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{database::DatabaseError, email::blocklist::ApprovedEmailAddress};

pub struct EmailSetupRequestRow {
    pub token: Uuid,
    pub developer_id: i32,
    pub email: String,
    password: String,
}

impl EmailSetupRequestRow {
    pub fn password(&self) -> Result<PasswordHash<'_>, password_hash::Error> {
        PasswordHash::new(&self.password)
    }
}

#[tracing::instrument(skip_all)]
pub async fn find_for_developer(
    developer_id: i32,
    conn: &mut PgConnection,
) -> Result<Option<EmailSetupRequestRow>, DatabaseError> {
    sqlx::query!(
        "SELECT developer_id, email, password, token
        FROM email_setup_requests
        WHERE developer_id = $1
        AND expires_at > NOW()",
        developer_id
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map_err(|e| e.into())
    .map(|result| {
        result.map(|e| EmailSetupRequestRow {
            developer_id: e.developer_id,
            token: e.token,
            email: e.email,
            password: e.password,
        })
    })
}

#[tracing::instrument(skip_all)]
pub async fn find_for_token(
    token: Uuid,
    conn: &mut PgConnection,
) -> Result<Option<EmailSetupRequestRow>, DatabaseError> {
    sqlx::query!(
        "SELECT developer_id, email, password, token
        FROM email_setup_requests
        WHERE token = $1",
        token
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map_err(|e| e.into())
    .map(|result| {
        result.map(|e| EmailSetupRequestRow {
            developer_id: e.developer_id,
            token: e.token,
            email: e.email,
            password: e.password,
        })
    })
}

#[tracing::instrument(skip_all, fields(developer_id = %developer_id))]
pub async fn create(
    uuid: Uuid,
    developer_id: i32,
    email: &ApprovedEmailAddress,
    password: PasswordHashString,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    let expires_at = Utc::now() + chrono::Duration::minutes(30);

    sqlx::query!(
        "INSERT INTO email_setup_requests
        (developer_id, email, password, expires_at, token)
        VALUES
        ($1, $2, $3, $4, $5)",
        developer_id,
        email.email().to_string(),
        password.as_str(),
        expires_at,
        uuid
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map(|_| ())
    .map_err(|e| e.into())
}

#[tracing::instrument(skip_all)]
pub async fn delete(token: &Uuid, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!("DELETE FROM email_setup_requests WHERE token = $1", token)
        .execute(&mut *conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))
        .map(|_| ())
        .map_err(|e| e.into())
}

#[tracing::instrument(skip_all, fields(developer_id = %developer_id))]
pub async fn delete_for_developer(
    developer_id: i32,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM email_setup_requests WHERE developer_id = $1",
        developer_id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map(|_| ())
    .map_err(|e| e.into())
}
