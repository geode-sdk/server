use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct ModUnlistHistory {
    pub id: i64,
    pub mod_id: String,
    pub unlisted: bool,
    pub details: Option<String>,
    pub modified_by: i32,
    pub created_at: DateTime<Utc>
}