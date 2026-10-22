use std::fmt::Display;

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

impl ModVersionStatusEnum {
    pub fn can_transition_to(self, to: ModVersionStatusEnum) -> bool {
        use ModVersionStatusEnum::*;

        match (self, to) {
            (Pending, Accepted | Rejected) => true,
            (Accepted, Unlisted) => true,
            (Rejected, Accepted) => true,
            (Unlisted, Accepted) => true,
            (Pending | Accepted | Rejected | Unlisted, _) => false,
        }
    }
}

impl Display for ModVersionStatusEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ModVersionStatusEnum::*;

        f.write_str(match self {
            Pending => "pending",
            Accepted => "accepted",
            Rejected => "rejected",
            Unlisted => "unlisted",
        })
    }
}
