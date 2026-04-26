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
pub struct SelfDeveloper {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub admin: bool,
    pub github_id: i64,
    pub has_accepted_mod: bool
}

impl Developer {
    pub fn to_self_developer(&self, has_accepted_mod: bool) -> SelfDeveloper {
        SelfDeveloper {
            id: self.id,
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            verified: self.verified,
            admin: self.admin,
            github_id: self.github_id,
            has_accepted_mod
        }
    }
}
