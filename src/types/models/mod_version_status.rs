use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, Hash, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase", type_name = "mod_version_status")]
pub enum ModVersionStatusEnum {
    Pending,
    Accepted,
    Rejected,
    Unlisted,
}
