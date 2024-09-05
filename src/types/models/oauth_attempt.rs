use std::time::Duration;

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct OAuthAttempt {
    pub uid: Uuid,
    pub interval: i32,
    pub expires_in: i32,
    #[serde(skip_serializing)]
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing)]
    pub last_poll: DateTime<Utc>,
    #[serde(skip_serializing)]
    pub token: Option<String>,
    #[serde(skip_serializing)]
    pub refresh_token: Option<String>,
}

impl OAuthAttempt {
    pub fn too_fast(&self) -> bool {
        let now = Utc::now();
        let diff = (now - self.last_poll).num_seconds() as i32;
        diff < self.interval
    }

    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let exprire_time =
            self.created_at + Duration::from_secs(u64::try_from(self.expires_in).unwrap());
        now > exprire_time
    }
}
