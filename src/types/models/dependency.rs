use serde::{Deserialize, Serialize};

#[derive(sqlx::FromRow)]
pub struct Dependency {
    dependent_id: i32,
    dependency_id: i32,
    compare: ModVersionCompare,
    importance: DependencyImportance
} 

#[derive(sqlx::FromRow)]
pub struct Incompatibility {
    mod_id: i32,
    incompatibility_id: i32,
    compare: ModVersionCompare,
    importance: DependencyImportance
}

#[derive(sqlx::Type, Debug, Deserialize, Serialize)]
#[sqlx(type_name = "version_compare")]
pub enum ModVersionCompare {
    #[serde(rename = "=")]
    #[sqlx(rename = "=")]
    Exact,
    #[serde(rename = ">")]
    #[sqlx(rename = ">")]
    More,
    #[serde(rename = ">=")]
    #[sqlx(rename = ">=")]
    MoreEq,
    #[serde(rename = "<")]
    #[sqlx(rename = "<")]
    Less,
    #[serde(rename = "=<")]
    #[sqlx(rename = "=<")]
    LessEq 
}

#[derive(sqlx::Type, Debug, Deserialize, Serialize)]
#[sqlx(type_name = "dependency_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DependencyImportance {
    Suggested,
    Recommended,
    Required
}