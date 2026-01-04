use crate::database::DatabaseError;
use crate::types::models::mod_unlist_history::ModUnlistHistory;
use sqlx::PgConnection;

pub async fn create(
    mod_id: &str,
    unlisted: bool,
    details: Option<String>,
    modified_by: i32,
    conn: &mut PgConnection,
) -> Result<ModUnlistHistory, DatabaseError> {
    sqlx::query_as!(
        ModUnlistHistory,
        "INSERT INTO mod_unlist_history
        (mod_id, unlisted, details, modified_by)
        VALUES ($1, $2, $3, $4)
        RETURNING
            id, mod_id, unlisted,
            details, modified_by, created_at",
        mod_id,
        unlisted,
        details,
        modified_by
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| log::error!("mod_unlist_history::create failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn get_last_for_mod(
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<Option<ModUnlistHistory>, DatabaseError> {
    sqlx::query_as!(
        ModUnlistHistory,
        "SELECT
            id, mod_id, unlisted,
            details, modified_by, created_at
        FROM mod_unlist_history
        WHERE mod_id = $1
        ORDER BY id DESC
        LIMIT 1",
        mod_id,
    )
        .fetch_optional(conn)
        .await
        .inspect_err(|e| log::error!("mod_unlist_history::get_last_for_mod failed: {e}"))
        .map_err(|e| e.into())
}

pub async fn get_for_mod(
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<Vec<ModUnlistHistory>, DatabaseError> {
    sqlx::query_as!(
        ModUnlistHistory,
        "SELECT
            id, mod_id, unlisted,
            details, modified_by, created_at
        FROM mod_unlist_history
        WHERE mod_id = $1
        ORDER BY id DESC",
        mod_id,
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| log::error!("mod_unlist_history::get_for_mod failed: {e}"))
    .map_err(|e| e.into())
}
