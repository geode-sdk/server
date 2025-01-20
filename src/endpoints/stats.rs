use actix_web::{get, web, Responder};

use crate::config::AppData;
use crate::types::{
    api::{ApiError, ApiResponse},
    models::stats::Stats,
};

#[get("/v1/stats")]
pub async fn get_stats(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data
        .db()
        .acquire()
        .await
        .or(Err(ApiError::DbAcquireError))?;
    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: Stats::get_cached(&mut pool).await?,
    }))
}
