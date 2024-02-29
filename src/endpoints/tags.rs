use actix_web::{get, web, Responder};

use crate::{
    types::{
        api::{ApiError, ApiResponse},
        models::tag::Tag,
    },
    AppData,
};

#[get("/v1/tags")]
pub async fn index(data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

    let tags = Tag::get_tags(&mut pool).await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: tags,
    }))
}
