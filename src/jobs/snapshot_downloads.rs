use sqlx::PgConnection;

pub async fn start(conn: &mut PgConnection) -> Result<(), String> {
    sqlx::query!(
        "INSERT INTO mod_versions_download_count_snapshots (mod_version_id, download_count)
        SELECT mv.id as mod_version_id, mv.download_count FROM mod_versions mv
        WHERE mv.download_count <> 0"
    )
    .execute(&mut *conn)
    .await
    .map_err(|err| format!("Query error: {}", err))?;

    sqlx::query!(
        "INSERT INTO mods_download_count_snapshots (mod_id, download_count)
        SELECT id as mod_id, download_count FROM mods WHERE download_count <> 0"
    )
    .execute(&mut *conn)
    .await
    .map_err(|err| format!("Query error: {}", err))?;

    // sqlx::query!(
    //     "DELETE FROM mod_downloads md
    //     USING mod_versions mv, mod_version_statuses mvs
    //     WHERE md.mod_version_id = mv.id
    //         AND mv.status_id = mvs.id
    //         AND mvs.status = 'accepted'
    //         AND md.time_downloaded < CURRENT_TIMESTAMP"
    // )
    // .execute(&mut *conn)
    // .await
    // .map_err(|err| format!("Query error: {}", err))?;

    Ok(())
}
