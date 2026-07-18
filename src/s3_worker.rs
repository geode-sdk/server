use std::time::Duration;

use actix_web::web;
use bytes::Bytes;

use crate::{
    config::AppData, database::repository::mod_versions::update_managed_download_link,
    types::models::mod_gd_version::GDVersionEnum,
};

pub enum S3WorkerTask {
    UploadMod {
        data: Bytes,
        mod_id: String,
        version: String,
        version_id: i32,
    },
}

fn path_for_mod(mod_id: &str, version: &str) -> String {
    format!("mods/{mod_id}/{version}/{mod_id}.geode")
}

async fn process_task(data: &AppData, task: S3WorkerTask) -> anyhow::Result<()> {
    let storage = data.mod_storage().expect("mod storage must be set by now");

    match task {
        S3WorkerTask::UploadMod {
            data: bytes,
            mod_id,
            version,
            version_id,
        } => {
            let path = path_for_mod(&mod_id, &version);
            let public_url = storage.asset_url(&path);

            storage.store(&path, &bytes).await?;

            let mut tx = data.db().begin().await?;
            update_managed_download_link(version_id, Some(&public_url), &mut tx).await?;
            tx.commit().await?;

            tracing::info!(
                "Uploaded mod {} {} to S3 at {}",
                mod_id,
                version,
                public_url
            );
        }
    }

    Ok(())
}

async fn cleanup_old_s3_files(data: &AppData) -> anyhow::Result<()> {
    // TODO
    Ok(())
}

async fn migrate_one(
    data: &AppData,
    original_url: &str,
    mod_id: &str,
    version: &str,
    version_id: i32,
) -> anyhow::Result<()> {
    let resp = data
        .http_client()
        .get(original_url)
        .send()
        .await?
        .error_for_status()?;
    let bytes = resp.bytes().await?;

    process_task(
        data,
        S3WorkerTask::UploadMod {
            data: bytes,
            mod_id: mod_id.to_owned(),
            version: version.to_owned(),
            version_id,
        },
    )
    .await
}

async fn migrate_existing_mods_to_s3(data: &AppData) -> anyhow::Result<()> {
    let supported_gd = GDVersionEnum::supported_for_storage();

    let mut db = data.db().acquire().await?;

    // gets the latest approved version of each mod for each supported GD version
    let versions = sqlx::query!(
        "SELECT final_q.id, final_q.version, final_q.download_link, final_q.mv_id FROM (
            SELECT DISTINCT ON (q.mv_id) q.id, q.version, q.download_link, q.mv_id, q.gd FROM (
                SELECT DISTINCT ON (m.id, mgv.gd) m.id, mv.name, mv.version, mv.download_link, mv.id as mv_id, mgv.gd
                FROM MODS m
                INNER JOIN mod_versions mv ON m.id = mv.mod_id
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
                WHERE mvs.status = 'accepted'
                AND mgv.gd = ANY($1::gd_version[])
                AND mv.managed_download_link IS NULL
                ORDER BY m.id, mgv.gd, mv.id DESC
            ) q
            ORDER BY q.mv_id, q.gd DESC
        ) final_q",
        supported_gd as &[GDVersionEnum]
    )
    .fetch_all(&mut *db)
    .await?;

    for record in versions {
        if let Err(e) = migrate_one(
            data,
            &record.download_link,
            &record.id,
            &record.version,
            record.mv_id,
        )
        .await
        {
            tracing::error!(
                "error migrating mod {} {} to S3: {e:?}",
                record.id,
                record.version
            );
        }
    }

    Ok(())
}

pub async fn run_s3_worker(data: web::Data<AppData>) {
    if data.mod_storage().is_none() {
        return;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    data.init_s3_sender(tx);

    let s_data = data.clone();
    tokio::spawn(async move {
        if let Err(e) = migrate_existing_mods_to_s3(&s_data).await {
            tracing::error!("Error migrating existing mods to S3: {:?}", e);
        }
    });

    let mut interval = tokio::time::interval(Duration::from_mins(30));

    loop {
        let result = tokio::select! {
            task = rx.recv() => match task {
                Some(task) => process_task(&data, task).await,
                None => break,
            },

            _ = interval.tick() => cleanup_old_s3_files(&data).await,
        };

        if let Err(e) = result {
            tracing::error!("Error processing S3 worker task: {:?}", e);
        }
    }
}
