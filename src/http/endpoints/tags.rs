use actix_web::{Responder, get, web};

use crate::config::AppData;
use crate::database::repository::mod_tags;
use crate::http::error::ApiError;
use crate::http::response::ApiResponse;
use crate::types::models::tag::Tag;

pub fn router_config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(index)
        .service(detailed_index);
}

/// Get all available tags
#[utoipa::path(
    tag = "tags",
    responses(
        (status = 200, description = "List of tag names", body = inline(ApiResponse<Vec<String>>))
    )
)]
#[get("/tags")]
#[tracing::instrument(skip_all)]
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

/// Get all available tags with detailed information
#[utoipa::path(
    tag = "tags",
    responses(
        (status = 200, description = "List of tags with details", body = inline(ApiResponse<Vec<Tag>>))
    )
)]
#[get("/detailed-tags")]
#[tracing::instrument(skip_all)]
pub async fn detailed_index(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db().acquire().await?;

    let tags = mod_tags::get_all(&mut pool).await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: tags,
    }))
}
