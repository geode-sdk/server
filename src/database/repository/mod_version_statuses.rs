use crate::database::DatabaseError;
use crate::types::models::mod_version_status::ModVersionStatusEnum;
use sqlx::PgConnection;

pub async fn create(
    mod_version_id: i32,
    status: ModVersionStatusEnum,
    info: Option<String>,
    conn: &mut PgConnection,
) -> Result<i32, DatabaseError> {
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
    .inspect_err(|e| log::error!("Failed to create mod_version_status: {e}"))
    .map(|i| i.id)
    .map_err(|e| e.into())
}
