use crate::types::api::ApiError;
use crate::types::models::developer::FetchedDeveloper;
use sqlx::PgConnection;

pub async fn get_owner_for_mod(
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<FetchedDeveloper, ApiError> {
    Ok(sqlx::query_as!(
        FetchedDeveloper,
        "SELECT
            dev.id,
            dev.username,
            dev.display_name,
            dev.verified,
            dev.admin
        FROM developers dev
        INNER JOIN mods_developers md ON md.developer_id = dev.id
        WHERE md.mod_id = $1
        AND md.is_owner = true",
        mod_id
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => {
            log::error!("Mod {} doesn't have an owner!", mod_id);
            ApiError::InternalError
        }
        _ => {
            log::error!("Failed to fetch owner for mod {}", mod_id);
            ApiError::InternalError
        }
    })?)
}
