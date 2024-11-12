use sqlx::PgConnection;

pub async fn start(conn: &mut PgConnection) -> Result<(), String> {
    sqlx::query!(
        "INSERT INTO mod_versions_download_count_snapshots (mod_version_id, download_count)
        SELECT id as mod_version_id, download_count FROM mod_versions"
    )
    .execute(&mut *conn)
    .await
    .map_err(|err| format!("{}", err))?;

    sqlx::query!(
        "INSERT INTO mods_download_count_snapshots (mod_id, download_count)
        SELECT id as mod_id, download_count FROM mods"
    )
    .execute(&mut *conn)
    .await
    .map_err(|err| format!("{}", err))?;

    Ok(())
}
