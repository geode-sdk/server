use serde::Serialize;

#[derive(sqlx::FromRow, Serialize, Clone)]
pub struct Deprecation {
    pub id: i32,
    pub mod_id: String,
    pub by: Vec<String>,
    pub reason: String,
}
