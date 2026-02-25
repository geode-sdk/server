use serde::Serialize;
use utoipa::ToSchema;

#[derive(sqlx::FromRow, Serialize, Clone, ToSchema)]
pub struct Deprecation {
    pub id: i32,
    pub mod_id: String,
    pub by: Vec<String>,
    pub reason: String,
}
