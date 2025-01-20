use crate::database::repository::{mod_gd_versions, mod_tags};
use crate::endpoints::tags;
use crate::types::api::ApiError;
use crate::types::mod_json::ModJson;
use crate::types::models::mod_gd_version::ModGDVersion;
use crate::types::models::mod_version::ModVersion;
use crate::types::models::tag::Tag;
use sqlx::PgConnection;

pub async fn create_from_json(
    json: &ModJson,
    conn: &mut PgConnection,
) -> Result<ModVersion, ApiError> {
    sqlx::query!("SET CONSTRAINTS public.mod_versions.mod_versions_status_id_fkey DEFERRED")
        .execute(conn)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to update constraints for mod_version creation: {}",
                e
            );
            ApiError::DbError
        })?;

    // status_id = 0 will be modified later
    let id = sqlx::query!(
        "INSERT INTO mod_versions
        (name, description, version, download_link, hash, geode,
        early_load, api, mod_id, status_id) VALUES
        ($1, $2, $3, $4, $5, $6,
        $7, $8, $9, 0)
        RETURNING id",
        json.name,
        json.description,
        json.version,
        json.download_url,
        json.hash,
        json.geode,
        json.early_load,
        json.api.is_some(),
        json.id
    )
    .fetch_one(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to create mod_version: {}", e);
        ApiError::DbError
    })?
    .id;

    let tags = mod_tags::get_all_from_names(json.tags.as_ref().unwrap_or_default(), conn).await?;
    mod_tags::update_for_mod(&json.id, tags.into_iter().map(|x| x.id).collect(), conn).await?;
    mod_gd_versions::create_from_json(&json.gd.to_create_payload(json), id, conn).await?;
}

pub async fn increment_downloads(id: i32, conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE mod_versions
        SET download_count = download_count + 1
        WHERE id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!(
            "Failed to increment downloads for mod_version {}: {}",
            id,
            e
        );
        ApiError::DbError
    })?;

    Ok(())
}
