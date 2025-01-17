use actix_web::{get, web, Responder};

use crate::{
    types::api::{ApiError, ApiResponse},
    AppData,
};

#[get("/v1/tags")]
pub async fn index(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    use crate::database::repository::*;

    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
    let tags = mod_tags::get_all(&mut pool).await?
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
    use crate::database::repository::*;
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let tags = mod_tags::get_all(&mut pool).await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: tags,
    }))
}
