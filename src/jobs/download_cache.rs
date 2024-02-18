use sqlx::PgConnection;

pub async fn start(pool: &mut PgConnection) -> Result<(), String> {
    // update mod_versions counts
    if let Err(e) = sqlx::query!(
        "UPDATE mod_versions mv SET download_count = mv.download_count + (
            SELECT COUNT(DISTINCT md.ip) FROM mod_downloads md
            WHERE md.mod_version_id = mv.id AND md.time_downloaded > mv.last_download_cache_refresh 
        ), last_download_cache_refresh = now()
        WHERE mv.validated = true"
    )
    .execute(&mut *pool)
    .await
    {
        log::error!("{}", e);
        return Err("Error updating mod version download count".to_string());
    }

    if let Err(e) = sqlx::query!(
        "UPDATE mods m SET download_count = m.download_count + (
            SELECT SUM(mv.download_count) FROM mod_versions mv
            WHERE mv.mod_id = m.id AND mv.validated = true
        ), last_download_cache_refresh = now()"
    )
    .execute(&mut *pool)
    .await
    {
        log::error!("{}", e);
        return Err("Error updating mod download count".to_string());
    }

    Ok(())
}
