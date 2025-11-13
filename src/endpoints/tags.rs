use actix_web::{get, web, Responder};

use crate::config::AppData;
use crate::database::repository::mod_tags;
use crate::endpoints::ApiError;
use crate::types::api::ApiResponse;

#[get("/v1/tags")]
pub async fn index(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db().acquire().await?;
    let tags = mod_tags::get_all_writable(&mut pool)
        .await?
        .into_iter()
        .map(|tag| tag.name)
        .collect::<Vec<String>>();

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: tags,
    }))
}

#[get("/v1/detailed-tags")]
pub async fn detailed_index(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db().acquire().await?;

    let tags = mod_tags::get_all(&mut pool).await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: tags,
    }))
}
