use sqlx::PgConnection;

use crate::database::DatabaseError;

#[tracing::instrument(skip_all, fields(tag = %tag, platform = %platform, url = %url, hash = %hash))]
pub async fn upsert_download(
    tag: &str,
    platform: &str,
    url: &str,
    hash: &str,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query(
        "INSERT INTO geode_version_download (tag, platform, url, hash) VALUES ($1, $2, $3, $4)
        ON CONFLICT (tag, platform) DO UPDATE SET url = $3, hash = $4",
    )
    .bind(tag)
    .bind(platform)
    .bind(url)
    .bind(hash)
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}

#[tracing::instrument(skip_all, fields(tag = %tag, url = %url, hash = %hash))]
pub async fn update_resources_download(
    tag: &str,
    url: &str,
    hash: &str,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query("UPDATE geode_versions SET resources_url = $2, resources_hash = $3 WHERE tag = $1")
        .bind(tag)
        .bind(url)
        .bind(hash)
        .execute(&mut *conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}
