use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use crate::types::api::ApiError;

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase", type_name = "mod_version_status")]
pub enum ModVersionStatusEnum {
    Pending,
    Accepted,
    Rejected,
    Unlisted,
}

pub struct ModVersionStatus {
    pub id: i32,
    pub mod_version_id: i32,
    pub status: ModVersionStatusEnum,
    pub info: Option<String>,
    pub admin_id: i32,
}

impl ModVersionStatus {
    pub async fn create_for_mod_version(
        id: i32,
        status: ModVersionStatusEnum,
        info: Option<String>,
        admin_id: Option<i32>,
        pool: &mut PgConnection,
    ) -> Result<i32, ApiError> {
        let result = sqlx::query!(
            "INSERT INTO mod_version_statuses (mod_version_id, status, info, admin_id) VALUES ($1, $2, $3, $4) RETURNING id",
            id,
            status as ModVersionStatusEnum,
            info,
            admin_id
        )
        .fetch_one(&mut *pool)
        .await;
        match result {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(r) => Ok(r.id),
        }
    }
}
