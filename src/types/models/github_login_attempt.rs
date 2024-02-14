use std::time::Duration;

use serde::Serialize;
use sqlx::{
    types::{
        chrono::{DateTime, Utc},
        ipnetwork::IpNetwork,
    },
    PgConnection,
};
use uuid::Uuid;

use crate::types::api::ApiError;

#[derive(Serialize)]
pub struct GithubLoginAttempt {
    pub uuid: String,
    pub interval: i32,
    pub uri: String,
    pub code: String,
}

pub struct StoredLoginAttempt {
    pub uuid: String,
    pub ip: IpNetwork,
    pub device_code: String,
    pub interval: i32,
    pub expires_in: i32,
    pub created_at: DateTime<Utc>,
    pub last_poll: DateTime<Utc>,
}

impl StoredLoginAttempt {
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let exprire_time =
            self.created_at + Duration::from_secs(u64::try_from(self.expires_in).unwrap());
        now > exprire_time
    }

    pub fn interval_passed(&self) -> bool {
        let now = Utc::now();
        let diff = (now - self.last_poll).num_seconds() as i32;
        diff > self.interval
    }
}

impl GithubLoginAttempt {
    pub async fn create(
        ip: IpNetwork,
        device_code: String,
        interval: i32,
        expires_in: i32,
        pool: &mut PgConnection,
    ) -> Result<Uuid, ApiError> {
        let result = sqlx::query!(
            "
            INSERT INTO github_login_attempts
            (ip, device_code, interval, expires_in) VALUES
            ($1, $2, $3, $4) RETURNING uid
            ",
            ip,
            device_code,
            interval,
            expires_in
        )
        .fetch_one(&mut *pool)
        .await;
        match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(u) => Ok(u.uid),
        }
    }

    pub async fn get_one(
        uuid: Uuid,
        pool: &mut PgConnection,
    ) -> Result<Option<StoredLoginAttempt>, ApiError> {
        let result = sqlx::query_as!(
            StoredLoginAttempt,
            "SELECT uid as uuid, ip, device_code, interval, expires_in, created_at, last_poll
            FROM github_login_attempts
            WHERE uid = $1",
            uuid
        )
        .fetch_optional(pool)
        .await;

        match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => Ok(r),
        }
    }

    pub async fn get_one_by_ip(
        ip: IpNetwork,
        pool: &mut PgConnection,
    ) -> Result<Option<StoredLoginAttempt>, ApiError> {
        let result = sqlx::query_as!(
            StoredLoginAttempt,
            "SELECT uid as uuid, ip, device_code, interval, expires_in, created_at, last_poll
            FROM github_login_attempts
            WHERE ip = $1",
            ip
        )
        .fetch_optional(pool)
        .await;

        match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => Ok(r),
        }
    }

    pub async fn remove(uuid: Uuid, pool: &mut PgConnection) {
        let _ = sqlx::query!("DELETE FROM github_login_attempts WHERE uid = $1", uuid)
            .execute(&mut *pool)
            .await;
    }

    pub async fn poll(uuid: Uuid, pool: &mut PgConnection) {
        let now = Utc::now();
        let _ = sqlx::query!(
            "UPDATE github_login_attempts SET last_poll = $1 WHERE uid = $2",
            now,
            uuid
        )
        .execute(&mut *pool)
        .await;
    }
}
