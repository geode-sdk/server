use actix_web::{Responder, get, web};

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
#[tracing::instrument(skip_all)]
pub async fn get_stats(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db().acquire().await?;
    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: Stats::get_cached(&mut pool).await?,
    }))
}
