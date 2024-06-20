use actix_web::{get, web, Responder};

use crate::types::api::{ApiError, ApiResponse};
use crate::types::models::stats::Stats;
use crate::AppData;

#[get("/v1/stats")]
pub async fn get_stats(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
	let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
	Ok(web::Json(ApiResponse {
		error: "".into(),
		payload: Stats::get_cached(&mut pool).await?,
	}))
}
