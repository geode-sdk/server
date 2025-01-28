use crate::types::api::ApiError;
use crate::types::models::github_login_attempt::StoredLoginAttempt;
use chrono::Utc;
use sqlx::types::ipnetwork::IpNetwork;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn get_one_by_ip(
    ip: IpNetwork,
    conn: &mut PgConnection,
) -> Result<Option<StoredLoginAttempt>, ApiError> {
    sqlx::query_as!(
        StoredLoginAttempt,
        "SELECT
                uid as uuid,
                ip,
                interval,
                expires_in,
                created_at,
                last_poll,
                challenge_uri as uri,
                device_code,
                user_code
            FROM github_login_attempts
            WHERE ip = $1",
        ip
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch existing login attempt: {}", e);
        ApiError::DbError
    })
}

pub async fn get_one_by_uuid(
    uuid: Uuid,
    pool: &mut PgConnection,
) -> Result<Option<StoredLoginAttempt>, ApiError> {
    sqlx::query_as!(
        StoredLoginAttempt,
        "SELECT
            uid as uuid,
            ip,
            interval,
            expires_in,
            created_at,
            last_poll,
            challenge_uri as uri,
            device_code,
            user_code
        FROM github_login_attempts
        WHERE uid = $1",
        uuid
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch GitHub login attempt: {}", e);
        ApiError::DbError
    })
}

pub async fn create(
    ip: IpNetwork,
    device_code: String,
    interval: i32,
    expires_in: i32,
    uri: &str,
    user_code: &str,
    pool: &mut PgConnection,
) -> Result<StoredLoginAttempt, ApiError> {
    sqlx::query_as!(
        StoredLoginAttempt,
        "INSERT INTO github_login_attempts
        (ip, device_code, interval, expires_in, challenge_uri, user_code) VALUES
        ($1, $2, $3, $4, $5, $6)
        RETURNING
            uid as uuid,
            ip,
            device_code,
            challenge_uri as uri,
            user_code,
            interval,
            expires_in,
            created_at,
            last_poll",
        ip,
        device_code,
        interval,
        expires_in,
        uri,
        user_code
    )
    .fetch_one(&mut *pool)
    .await
    .map_err(|e| {
        log::error!("Failed to insert new GitHub login attempt: {}", e);
        ApiError::DbError
    })
}

pub async fn poll_now(uuid: Uuid, conn: &mut PgConnection) -> Result<(), ApiError> {
    let now = Utc::now();
    sqlx::query!(
        "UPDATE github_login_attempts
        SET last_poll = $1
        WHERE uid = $2",
        now,
        uuid
    )
    .execute(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to poll GitHub login attempt: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}

pub async fn remove(uuid: Uuid, conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!("DELETE FROM github_login_attempts WHERE uid = $1", uuid)
        .execute(conn)
        .await
        .map_err(|e| {
            log::error!("Failed to remove GitHub login attempt: {}", e);
            ApiError::DbError
        })?;

    Ok(())
}
