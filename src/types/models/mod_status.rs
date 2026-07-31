use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, Hash, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase", type_name = "mod_status")]
pub enum ModStatusEnum {
    Default,
		Archived,
    Unlisted,
}
