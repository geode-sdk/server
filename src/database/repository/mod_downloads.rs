use crate::types::api::ApiError;
use chrono::{Days, Utc};
use sqlx::types::ipnetwork::IpNetwork;
use sqlx::PgConnection;

pub async fn create(
    ip: IpNetwork,
    mod_version_id: i32,
    conn: &mut PgConnection,
) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        "INSERT INTO mod_downloads (mod_version_id, ip)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING",
        mod_version_id,
        ip
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!(
            "Failed to insert new download for mod_version id {}: {}",
            mod_version_id,
            e
        );
        ApiError::DbError
    })?;

    Ok(result.rows_affected() > 0)
}

pub async fn has_downloaded_mod(
    ip: IpNetwork,
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<bool, ApiError> {
    Ok(sqlx::query!(
        "SELECT ip FROM mod_downloads md
        INNER JOIN mod_versions mv ON md.mod_version_id = mv.id
        WHERE mv.mod_id = $1
        AND md.ip = $2
        LIMIT 1",
        mod_id,
        ip
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to check if mod has been downloaded: {}", e);
        ApiError::DbError
    })?
    .is_some())
}

pub async fn cleanup(conn: &mut PgConnection) -> Result<(), ApiError> {
    let date = Utc::now().checked_sub_days(Days::new(30)).unwrap();
    sqlx::query!(
        "DELETE FROM mod_downloads md
        WHERE md.time_downloaded <= $1",
        date
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to cleanup downloads: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}
