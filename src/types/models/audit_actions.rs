use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::ToSchema;
use crate::types::serde::chrono_dt_secs;

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

#[derive(Serialize, Debug, Clone, ToSchema)]
pub struct AuditActionRow {
    pub action: AuditAction,
    pub details: Option<String>,
    pub performed_by: Option<i32>,
    #[serde(with = "chrono_dt_secs")]
    pub performed_at: DateTime<Utc>,
}
