use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::ToSchema;

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash, ToSchema)]
#[sqlx(type_name = "audit_action", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AuditAction {
    Created,
    Updated,
    Deleted,
    Restored,
}

impl FromStr for AuditAction {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "created" => Ok(AuditAction::Created),
            "updated" => Ok(AuditAction::Updated),
            "deleted" => Ok(AuditAction::Deleted),
            "restored" => Ok(AuditAction::Restored),
            _ => Err(()),
        }
    }
}
