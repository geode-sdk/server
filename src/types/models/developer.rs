use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct ModDeveloper {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub is_owner: bool,
}

#[derive(sqlx::FromRow, Serialize, Clone, Debug, ToSchema)]
pub struct Developer {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub admin: bool,
    pub github_id: i64,
}

#[derive(sqlx::FromRow, Serialize, Clone, Debug, ToSchema)]
pub struct DeveloperBan {
    pub developer_id: i32,
    pub reason: Option<String>,
    pub admin_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}
