use serde::{Deserialize, Serialize};

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy)]
#[sqlx(type_name = "mod_version_status")]
pub enum ModVersionStatusEnum {
    Pending,
    Accepted,
    Rejected,
    Unlisted,
}

pub struct ModVersionStatus {
    pub id: i32,
    pub mod_version_id: i32,
    pub status: ModVersionStatusEnum,
    pub info: Option<String>,
    pub admin_id: i32,
}
