use sqlx::Acquire;

use crate::AppData;

mod download_cache;
mod snapshot_downloads;

pub async fn start_job(name: &str, app_data: AppData) -> Result<(), String> {
    match name {
        "download-cache" => {
            let mut pool = app_data
                .db
                .acquire()
                .await
                .or(Err("Couldn't connect to database"))?;
            download_cache::start(&mut pool).await
        }
        "snapshot-downloads" => {
            let mut connection = app_data
                .db
                .acquire()
                .await
                .or(Err("Couldn't connect to database"))?;
            let mut transaction = connection
                .begin()
                .await
                .or(Err("Couldn't connect to database"))?;
            let res = snapshot_downloads::start(&mut transaction).await;
            if res.is_ok() {
                transaction.commit().await.map_err(|e| format!("{}", e))?;
            }
            res
        }
        _ => Err(format!("Job not found {}", name)),
    }
}
