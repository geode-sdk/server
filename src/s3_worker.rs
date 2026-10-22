use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use reqwest::StatusCode;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    config::AppData,
    database::repository::{
        geode_versions::{update_resources_download, upsert_download},
        mod_versions::update_managed_download_link,
        mods::update_mod_logo_url,
    },
    mod_zip,
    types::models::{
        loader_version::{LoaderDownload, LoaderDownloads},
        mod_gd_version::GDVersionEnum,
    },
};

pub(crate) struct QueuedS3Task {
    pub task: S3WorkerTask,
    pub request_id: Option<Uuid>,
}

pub enum S3WorkerTask {
    UploadMod {
        data: Bytes,
        mod_id: String,
        version: String,
        version_id: i32,
    },

    UploadLoader {
        tag: String,
    },
}

fn installer_ext_for_platform(platform: &str) -> &'static str {
    match platform {
        "win" => "exe",
        "mac" => "pkg",
        "linux" => "sh",
        _ => panic!("Unsupported platform for installer: {}", platform),
    }
}

fn path_for_mod(mod_id: &str, version: &str) -> String {
    format!("mods/{mod_id}/{version}/{mod_id}.geode")
}

fn path_for_loader(tag: &str, platform: &str) -> String {
    format!("geode/{tag}/geode-v{tag}-{platform}.zip")
}

fn path_for_installer(tag: &str, platform: &str) -> String {
    let ext = installer_ext_for_platform(platform);
    format!("geode/{tag}/geode-installer-v{tag}-{platform}.{ext}")
}

fn path_for_resources(tag: &str) -> String {
    format!("geode/{tag}/resources.zip")
}

fn github_url_for_loader(tag: &str, platform: &str) -> String {
    format!(
        "https://github.com/geode-sdk/geode/releases/download/v{tag}/geode-v{tag}-{platform}.zip"
    )
}

fn github_url_for_installer(tag: &str, platform: &str) -> String {
    let ext = installer_ext_for_platform(platform);
    format!(
        "https://github.com/geode-sdk/geode/releases/download/v{tag}/geode-installer-v{tag}-{platform}.{ext}"
    )
}

fn github_url_for_resources(tag: &str) -> String {
    format!("https://github.com/geode-sdk/geode/releases/download/v{tag}/resources.zip")
}

struct LoaderMigration {
    github_url: String,
    path: String,
    download_name: String,
    tag: String,
    resources: bool,
    mime_type: String,
}

impl LoaderMigration {
    fn new(tag: &str, platform: &str) -> Self {
        LoaderMigration {
            github_url: github_url_for_loader(tag, platform),
            path: path_for_loader(tag, platform),
            tag: tag.to_owned(),
            download_name: platform.to_owned(),
            resources: false,
            mime_type: "application/zip".to_owned(),
        }
    }

    fn new_installer(tag: &str, platform: &str) -> Self {
        LoaderMigration {
            github_url: github_url_for_installer(tag, platform),
            path: path_for_installer(tag, platform),
            tag: tag.to_owned(),
            resources: false,
            download_name: format!("{platform}-installer"),
            mime_type: "application/octet-stream".to_owned(),
        }
    }

    fn new_resources(tag: &str) -> Self {
        LoaderMigration {
            github_url: github_url_for_resources(tag),
            path: path_for_resources(tag),
            tag: tag.to_owned(),
            resources: true,
            download_name: "resources".to_owned(),
            mime_type: "application/zip".to_owned(),
        }
    }
}

async fn migrate_artifact(
    data: &AppData,
    db: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    mig: &LoaderMigration,
) -> anyhow::Result<Option<LoaderDownload>> {
    let storage = data.cdn_storage().expect("mod storage must be set by now");

    let resp = data.http_client().get(&mig.github_url).send().await?;
    if resp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let bytes = resp.error_for_status()?.bytes().await?;
    let public_url = storage.asset_url(&mig.path);
    let hash = sha256::digest(&bytes[..]);
    storage.store(&mig.path, &bytes, &mig.mime_type).await?;

    if mig.resources {
        update_resources_download(&mig.tag, &public_url, &hash, db).await?;
    } else {
        upsert_download(&mig.tag, &mig.download_name, &public_url, &hash, db).await?;
    }

    Ok(Some(LoaderDownload {
        url: public_url,
        hash,
    }))
}

async fn migrate_geode_version(
    data: &AppData,
    db: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tag: &str,
    platform: &str,
) -> anyhow::Result<LoaderDownload> {
    match migrate_artifact(data, db, &LoaderMigration::new(tag, platform)).await? {
        Some(download) => Ok(download),
        None => Err(anyhow::anyhow!(
            "Geode version {} for platform '{}' not found on GitHub",
            tag,
            platform
        )),
    }
}

async fn migrate_geode_installer(
    data: &AppData,
    db: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tag: &str,
    platform: &str,
) -> anyhow::Result<LoaderDownload> {
    match migrate_artifact(data, db, &LoaderMigration::new_installer(tag, platform)).await? {
        Some(download) => Ok(download),
        None => Err(anyhow::anyhow!(
            "Geode {} installer for platform '{}' not found on GitHub",
            tag,
            platform
        )),
    }
}

async fn migrate_resources(
    data: &AppData,
    db: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tag: &str,
) -> anyhow::Result<LoaderDownload> {
    match migrate_artifact(data, db, &LoaderMigration::new_resources(tag)).await? {
        Some(download) => Ok(download),
        None => Err(anyhow::anyhow!("Resources for {} not found on GitHub", tag,)),
    }
}

fn path_for_mod_logo(mod_id: &str) -> String {
    format!("mods/{mod_id}/logo.png")
}

async fn upload_mod_logo(
    data: &AppData,
    mod_id: &str,
    current_logo: Option<Vec<u8>>,
    db: &mut PgConnection,
) -> anyhow::Result<()> {
    let storage = data.cdn_storage().expect("mod storage must be set by now");

    let logo_path = path_for_mod_logo(mod_id);
    let logo_public_url = storage.asset_url(&logo_path);

    let current_logo = match current_logo {
        Some(logo) => Some(logo),
        _ => sqlx::query!("SELECT image FROM mods WHERE id = $1", mod_id)
            .fetch_optional(&mut *db)
            .await?
            .and_then(|r| r.image),
    };

    if let Some(logo_bytes) = current_logo {
        storage.store(&logo_path, &logo_bytes, "image/png").await?;

        update_mod_logo_url(mod_id, &logo_public_url, db).await?;

        tracing::info!(url = %logo_public_url, "uploaded mod logo");
    }

    Ok(())
}

#[tracing::instrument(
    name = "s3_task",
    skip_all,
    fields(
        request_id = request_id.map(tracing::field::display),
        is_migration = is_migration,
        kind = tracing::field::Empty,
        mod_id = tracing::field::Empty,
        version = tracing::field::Empty,
        version_id = tracing::field::Empty,
        size = tracing::field::Empty,
        tag = tracing::field::Empty,
    ),
    err(Debug)
)]
async fn process_task(
    data: &AppData,
    task: S3WorkerTask,
    is_migration: bool,
    request_id: Option<Uuid>,
) -> anyhow::Result<()> {
    let span = tracing::Span::current();

    match task {
        S3WorkerTask::UploadMod {
            data: bytes,
            mod_id,
            version,
            version_id,
        } => {
            span.record("kind", "upload_mod")
                .record("mod_id", mod_id.as_str())
                .record("version", version.as_str())
                .record("version_id", version_id)
                .record("size", bytes.len());

            upload_mod(data, bytes, &mod_id, &version, version_id, is_migration).await
        }

        S3WorkerTask::UploadLoader { tag } => {
            span.record("kind", "upload_loader")
                .record("tag", tag.as_str());

            upload_loader(data, &tag).await
        }
    }
}

async fn upload_mod(
    data: &AppData,
    bytes: Bytes,
    mod_id: &str,
    version: &str,
    version_id: i32,
    is_migration: bool,
) -> anyhow::Result<()> {
    let storage = data.cdn_storage().expect("mod storage must be set by now");
    let mut db = data.db().acquire().await?;

    let path = path_for_mod(mod_id, version);
    let public_url = storage.asset_url(&path);

    storage
        .store(&path, &bytes, "application/octet-stream")
        .await?;

    update_managed_download_link(version_id, Some(&public_url), &mut db).await?;

    // upload logo if not migrating mods
    if !is_migration {
        upload_mod_logo(data, mod_id, None, &mut db).await?;
    }

    tracing::info!(url = %public_url, "uploaded mod");

    Ok(())
}

async fn upload_loader(data: &AppData, tag: &str) -> anyhow::Result<()> {
    tracing::info!("uploading loader release");

    let mut tx = data.db().begin().await?;

    let downloads = LoaderDownloads {
        win: migrate_geode_version(data, &mut tx, tag, "win").await?,
        mac: migrate_geode_version(data, &mut tx, tag, "mac").await?,
        android32: migrate_geode_version(data, &mut tx, tag, "android32").await?,
        android64: migrate_geode_version(data, &mut tx, tag, "android64").await?,
        ios: migrate_artifact(data, &mut tx, &LoaderMigration::new(tag, "ios"))
            .await?
            .unwrap_or_default(),
        resources: migrate_resources(data, &mut tx, tag).await?,

        win_installer: migrate_geode_installer(data, &mut tx, tag, "win").await?,
        mac_installer: migrate_geode_installer(data, &mut tx, tag, "mac").await?,
        linux_installer: migrate_geode_installer(data, &mut tx, tag, "linux").await?,
    };

    tx.commit().await?;

    tracing::info!("uploaded loader release");
    tracing::debug!(?downloads, "loader release downloads");

    Ok(())
}

#[tracing::instrument(name = "s3_cleanup", skip_all, err(Debug))]
async fn cleanup_old_s3_files(data: &AppData) -> anyhow::Result<()> {
    let supported_gd = GDVersionEnum::supported_for_storage();
    let storage = data.cdn_storage().expect("mod storage must be set by now");

    let versions = sqlx::query!(
        "SELECT mv.id, mv.version, mv.managed_download_link, mv.mod_id FROM mod_versions mv
        WHERE managed_download_link IS NOT NULL
            AND mv.id NOT IN (
                SELECT DISTINCT ON (q.mv_id) q.mv_id FROM (
                    SELECT DISTINCT ON (m.id, mgv.gd, mv.geode_major) m.id, mv.name, mv.version, mv.download_link, mv.managed_download_link, mv.id as mv_id, mgv.gd
                    FROM MODS m
                    INNER JOIN mod_versions mv ON m.id = mv.mod_id
                    INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                    INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
                    WHERE mvs.status = 'accepted' AND mgv.gd = ANY($1::gd_version[])
                    ORDER BY m.id, mgv.gd DESC, mv.geode_major DESC, mv.id DESC
                ) q
                ORDER BY q.mv_id
            )
        ",
        supported_gd as &[GDVersionEnum]
    )
    .fetch_all(&mut *data.db().acquire().await?)
    .await?;

    if versions.is_empty() {
        return Ok(());
    }

    tracing::info!(total = versions.len(), "cleaning up old S3 files");

    for record in versions {
        let path = path_for_mod(&record.mod_id, &record.version);
        if let Err(e) = storage.delete(&path).await {
            tracing::error!(error = ?e, mod_id = %record.mod_id, version = %record.version, %path, "failed to delete old S3 file");
            continue;
        }

        tracing::debug!(%path, "deleted old S3 file");

        let mut tx = data.db().begin().await?;
        update_managed_download_link(record.id, None, &mut tx).await?;
        tx.commit().await?;
    }

    Ok(())
}

#[tracing::instrument(
    name = "s3_migrate_mod",
    skip_all,
    fields(mod_id = %mod_id, version = %version, version_id = version_id),
    err(Debug)
)]
async fn migrate_one(
    data: &AppData,
    original_url: &str,
    mod_id: &str,
    version: &str,
    version_id: i32,
) -> anyhow::Result<()> {
    let bytes = mod_zip::download_mod(
        data.check_dns_http_client(),
        original_url,
        data.max_download_mb(),
    )
    .await?;

    upload_mod(data, bytes, mod_id, version, version_id, true).await
}

#[tracing::instrument(name = "s3_migrate_logo", skip_all, fields(mod_id = %mod_id), err(Debug))]
async fn migrate_mod_logo(
    data: &AppData,
    mod_id: &str,
    image: Option<Vec<u8>>,
    db: &mut PgConnection,
) -> anyhow::Result<()> {
    upload_mod_logo(data, mod_id, image, db).await
}

#[tracing::instrument(name = "s3_migrate_mods", skip_all, err(Debug))]
async fn migrate_existing_mods_to_s3(data: &AppData) -> anyhow::Result<()> {
    let supported_gd = GDVersionEnum::supported_for_storage();

    // gets the latest approved version of each mod for each supported GD version
    let versions = sqlx::query!(
        "SELECT final_q.id, final_q.version, final_q.download_link, final_q.mv_id FROM (
            SELECT DISTINCT ON (q.mv_id) q.id, q.version, q.download_link, q.mv_id, q.gd FROM (
                SELECT DISTINCT ON (m.id, mgv.gd, mv.geode_major) m.id, mv.name, mv.version, mv.download_link, mv.managed_download_link, mv.id as mv_id, mgv.gd
                FROM MODS m
                INNER JOIN mod_versions mv ON m.id = mv.mod_id
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
                WHERE mvs.status = 'accepted' AND mgv.gd = ANY($1::gd_version[])
                ORDER BY m.id, mgv.gd DESC, mv.geode_major DESC, mv.id DESC
            ) q
            WHERE q.managed_download_link IS NULL
            ORDER BY q.mv_id, q.gd DESC
        ) final_q",
        supported_gd as &[GDVersionEnum]
    )
    .fetch_all(&mut *data.db().acquire().await?)
    .await?;

    if !versions.is_empty() {
        let total = versions.len();
        let mut failed = 0;

        tracing::info!(total = versions.len(), "migrating existing mods to S3");

        for record in versions {
            if migrate_one(
                data,
                &record.download_link,
                &record.id,
                &record.version,
                record.mv_id,
            )
            .await
            .is_err()
            {
                failed += 1;
            }
        }

        tracing::info!(total, failed, "finished migrating mods");
    }

    // independently migrate mod logos
    let mut db = data.db().acquire().await?;
    let mods = sqlx::query!(
        "SELECT id, image FROM mods WHERE image IS NOT NULL AND length(image) > 0 AND image_url IS NULL"
    )
    .fetch_all(&mut *db)
    .await?;

    if !mods.is_empty() {
        let total = mods.len();
        let mut failed = 0;

        tracing::info!(total = mods.len(), "migrating mod logos to S3");

        for record in mods {
            if migrate_mod_logo(data, &record.id, record.image, &mut db)
                .await
                .is_err()
            {
                failed += 1;
            }
        }

        tracing::info!(total, failed, "finished migrating mod logos");
    }

    Ok(())
}

#[tracing::instrument(name = "s3_migrate_loader", skip_all, err(Debug))]
async fn migrate_loader_versions_to_s3(data: &AppData) -> anyhow::Result<()> {
    let versions: Vec<String> = sqlx::query_scalar(
        "SELECT gv.tag FROM geode_versions gv WHERE NOT EXISTS (
            SELECT 1 FROM geode_version_download gvd WHERE gvd.tag = gv.tag
        )",
    )
    .fetch_all(&mut *data.db().acquire().await?)
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    if !versions.is_empty() {
        let total = versions.len();
        let mut failed = 0;

        tracing::info!(total = versions.len(), "migrating Geode releases");

        for tag in versions {
            if process_task(data, S3WorkerTask::UploadLoader { tag }, true, None)
                .await
                .is_err()
            {
                failed += 1;
            }
        }

        tracing::info!(total, failed, "finished migrating Geode releases");
    }

    Ok(())
}

pub async fn run_s3_worker(data: Arc<AppData>) {
    if data.cdn_storage().is_none() {
        return;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    data.init_s3_sender(tx);

    let s_data = data.clone();
    tokio::spawn(async move {
        let _ = migrate_existing_mods_to_s3(&s_data).await;
    });

    let s_data2 = data.clone();
    tokio::spawn(async move {
        let _ = migrate_loader_versions_to_s3(&s_data2).await;
    });

    let mut interval = tokio::time::interval(Duration::from_mins(30));

    loop {
        tokio::select! {
            queued = rx.recv() => {
                let Some(queued) = queued else { break };
                let _ = process_task(&data, queued.task, false, queued.request_id).await;
            }

            _ = interval.tick() => {
                let _ = cleanup_old_s3_files(&data).await;
            },
        };
    }
}
