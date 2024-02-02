use std::net::Ipv4Addr;

use serde::Serialize;
use sqlx::{types::{chrono::{self, DateTime, Local, TimeZone, Utc}, ipnetwork::IpNetwork}, PgConnection};
use uuid::Uuid;

use crate::types::api::ApiError;

#[derive(Serialize)]
pub struct GithubLoginAttempt {
    pub uuid: String,
    pub interval: i32,
    pub uri: String,
    pub code: String
}

pub struct StoredLoginAttempt {
    uuid: String,
    ip: IpNetwork,
    device_code: String,
    interval: i32,
    expires_in: i32,
    created_at: DateTime<Utc>,
    last_poll: Option<DateTime<Utc>>
}

impl GithubLoginAttempt {
    pub async fn create(
        ip: IpNetwork,
        device_code: String,
        interval: i32,
        expires_in: i32,
        pool: &mut PgConnection
    ) -> Result<Uuid, ApiError>{
        let result = sqlx::query!("
            INSERT INTO github_login_attempts
            (ip, device_code, interval, expires_in) VALUES
            ($1, $2, $3, $4) RETURNING uid
            ", 
        ip, device_code, interval, expires_in)
            .fetch_one(&mut *pool)
            .await;
        match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            },
            Ok(u) => Ok(u.uid)
        }
    }

    pub async fn get_one(uuid: Uuid, pool: &mut PgConnection) -> Result<Option<StoredLoginAttempt>, ApiError> {
        let result = sqlx::query_as!(StoredLoginAttempt,
            "SELECT uid as uuid, ip, device_code, interval, expires_in, created_at, last_poll
            FROM github_login_attempts
            WHERE uid = $1", uuid
        ).fetch_optional(pool).await;

        match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            },
            Ok(r) => Ok(r)
        }
    }

    pub async fn get_one_by_ip(ip: IpNetwork, pool: &mut PgConnection) -> Result<Option<StoredLoginAttempt>, ApiError> {
        let result = sqlx::query_as!(StoredLoginAttempt,
            "SELECT uid as uuid, ip, device_code, interval, expires_in, created_at, last_poll
            FROM github_login_attempts
            WHERE ip = $1", ip 
        ).fetch_optional(pool).await;

        match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            },
            Ok(r) => Ok(r)
        }
    }
}