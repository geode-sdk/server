use crate::AppData;

mod download_cache;

pub async fn start_job(name: &str, app_data: AppData) -> Result<(), String> {
    match name {
        "download_cache" => {
            let mut pool = app_data
                .db
                .acquire()
                .await
                .or(Err("Couldn't connect to database"))?;
            download_cache::start(&mut pool).await
        }
        _ => Err(format!("Job not found {}", name)),
    }
}
