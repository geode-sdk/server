use sqlx::PgConnection;

use crate::types::{api::ApiError, models::mod_version_status::ModVersionStatusEnum};

pub async fn create(
    mod_version_id: i32,
    status: ModVersionStatusEnum,
    info: Option<String>,
    conn: &mut PgConnection,
) -> Result<i32, ApiError> {
    sqlx::query!(
        "INSERT INTO mod_version_statuses
        (mod_version_id, status, info, admin_id)
        VALUES ($1, $2, $3, NULL)
        RETURNING id",
        mod_version_id,
        status as ModVersionStatusEnum,
        info
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| log::error!("Failed to create status: {}", e))
    .or(Err(ApiError::DbError))
    .map(|i| i.id)
}
