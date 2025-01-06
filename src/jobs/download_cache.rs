use sqlx::PgConnection;

pub async fn start(pool: &mut PgConnection) -> Result<(), String> {
    // update mod_versions counts
    if let Err(e) = sqlx::query!(
        "UPDATE mod_versions mv
        USING
        SET download_count = COALESCE((
            SELECT COUNT(DISTINCT md.ip) FROM mod_downloads md
            WHERE md.mod_version_id = mv.id
        ), 0) + COALESCE((
            SELECT download_count FROM mod_versions_download_count_snapshots
            WHERE mod_version_id = mv.id
            ORDER BY id DESC
        ), 0), last_download_cache_refresh = now()
        FROM mod_version_statuses mvs
        WHERE mv.status_id = mvs.id AND mvs.status = 'accepted'"
    )
    .execute(&mut *pool)
    .await
    {
        log::error!("{}", e);
        return Err("Error updating mod version download count".to_string());
    }

    if let Err(e) = sqlx::query!(
        "UPDATE mods m SET download_count = COALESCE((
            SELECT COUNT(DISTINCT md.ip) FROM mod_downloads md
            INNER JOIN mod_versions mv ON md.mod_version_id = mv.id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE mv.mod_id = m.id AND mvs.status = 'accepted'
        ), 0), last_download_cache_refresh = now()
        WHERE m.id IN (
            SELECT DISTINCT mv.mod_id FROM mod_versions mv 
            INNER JOIN mod_version_statuses mvs ON mv.status_id = mvs.id
            WHERE mvs.status = 'accepted'
        )"
    )
    .execute(&mut *pool)
    .await
    {
        log::error!("{}", e);
        return Err("Error updating mod download count".to_string());
    }

    Ok(())
}
