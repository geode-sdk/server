use sqlx::PgConnection;
use uuid::Uuid;

use crate::types::{api::ApiError, models::oauth_attempt::OAuthAttempt};

pub async fn find_one(
    uuid: &Uuid,
    connection: &mut PgConnection,
) -> Result<Option<OAuthAttempt>, ApiError> {
    sqlx::query_as!(
        OAuthAttempt,
        "
            SELECT 
                uid,
                interval,
                expires_in,
                created_at, 
                last_poll,
                token,
                refresh_token
            FROM oauth_attempts
            WHERE uid = $1
        ",
        uuid
    )
    .fetch_optional(connection)
    .await
    .map_err(|x| {
        log::error!("{}", x);
        ApiError::DbError
    })
}

pub async fn create(secret: Uuid, connection: &mut PgConnection) -> Result<OAuthAttempt, ApiError> {
    sqlx::query_as!(
        OAuthAttempt,
        "
            INSERT INTO oauth_attempts
            (uid, interval, expires_in)
            VALUES ($1, $2, $3)
            RETURNING 
                uid,
                interval,
                expires_in,
                created_at,
                last_poll,
                token,
                refresh_token
        ",
        secret,
        5,
        15 * 60
    )
    .fetch_one(connection)
    .await
    .map_err(|x| {
        log::error!("{}", x);
        ApiError::DbError
    })
}

pub async fn update_poll(
    uuid: &Uuid,
    connection: &mut PgConnection,
) -> Result<OAuthAttempt, ApiError> {
    sqlx::query_as!(
        OAuthAttempt,
        "
            UPDATE oauth_attempts
            SET last_poll = now()
            WHERE uid = $1
            RETURNING
                uid,
                interval,
                expires_in,
                created_at,
                last_poll,
                token,
                refresh_token
        ",
        uuid
    )
    .fetch_one(connection)
    .await
    .map_err(|x| {
        log::error!("{}", x);
        ApiError::DbError
    })
}

pub async fn set_tokens(
    uuid: &Uuid,
    token: &str,
    refresh_token: &str,
    connection: &mut PgConnection,
) -> Result<OAuthAttempt, ApiError> {
    sqlx::query_as!(
        OAuthAttempt,
        "
            UPDATE oauth_attempts
            SET token = $1, refresh_token = $2
            WHERE uid = $3
            RETURNING
                uid,
                interval,
                expires_in,
                created_at,
                last_poll,
                token,
                refresh_token
        ",
        token,
        refresh_token,
        uuid
    )
    .fetch_one(connection)
    .await
    .map_err(|x| {
        log::error!("{}", x);
        ApiError::DbError
    })
}

pub async fn delete(uid: Uuid, connection: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!(
        "
            DELETE FROM oauth_attempts
            WHERE uid = $1
        ",
        uid
    )
    .execute(connection)
    .await
    .map(|_| ())
    .map_err(|x| {
        log::error!("{}", x);
        ApiError::DbError
    })
}
