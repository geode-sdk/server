use crate::types::api::ApiError;
use sqlx::PgConnection;

pub async fn exists(id: &str, conn: &mut PgConnection) -> Result<bool, ApiError> {
    Ok(sqlx::query!("SELECT id FROM mods WHERE id = $1", id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| {
            log::error!("Failed to check if mod {} exists: {}", id, e);
            ApiError::DbError
        })?
        .is_some())
}

pub async fn get_logo(id: &str, conn: &mut PgConnection) -> Result<Option<Vec<u8>>, ApiError> {
    struct QueryResult {
        image: Option<Vec<u8>>,
    }

    let vec = sqlx::query_as!(
        QueryResult,
        "SELECT
            m.image
        FROM mods m
        INNER JOIN mod_versions mv ON mv.mod_id = m.id
        INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
        WHERE m.id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch mod logo for {}: {}", id, e);
        ApiError::DbError
    })?
    .map(|optional| optional.image)
    .flatten();

    // Empty vec is basically no image
    if vec.as_ref().is_some_and(|v| v.is_empty()) {
        Ok(None)
    } else {
        Ok(vec)
    }
}
