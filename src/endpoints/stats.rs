use actix_web::{get, web, Responder};

use super::ApiError;
use crate::config::AppData;
use crate::types::{api::ApiResponse, models::stats::Stats};

/// Get global index statistics
#[utoipa::path(
    get,
    path = "/v1/stats",
    tag = "stats",
    responses(
        (status = 200, description = "Index statistics", body = inline(ApiResponse<Stats>))
    )
)]
#[get("/v1/stats")]
pub async fn get_stats(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db().acquire().await?;
    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: Stats::get_cached(&mut pool).await?,
    }))
}
