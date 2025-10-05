use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ModDeveloper {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub is_owner: bool,
}

#[derive(sqlx::FromRow, Serialize, Clone, Debug)]
pub struct Developer {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub admin: bool,
    pub superadmin: bool,
    pub github_id: i64,
}
