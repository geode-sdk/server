use std::time::Duration;

use serde::Serialize;
use sqlx::types::{
    chrono::{DateTime, Utc},
    ipnetwork::IpNetwork,
};

#[derive(Serialize)]
pub struct StoredLoginAttempt {
    pub uuid: String,
    #[serde(skip_serializing)]
    pub ip: IpNetwork,
    #[serde(skip_serializing)]
    pub device_code: String,
    pub uri: String,
    #[serde(rename = "code")]
    pub user_code: String,
    pub interval: i32,
    #[serde(skip_serializing)]
    pub expires_in: i32,
    #[serde(skip_serializing)]
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing)]
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
