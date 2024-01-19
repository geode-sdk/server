use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Postgres, PgConnection};

use crate::types::api::ApiError;

#[derive(sqlx::FromRow)]
pub struct Dependency {
    pub dependent_id: i32,
    pub dependency_id: i32,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance
} 

#[derive(sqlx::FromRow)]
pub struct Incompatibility {
    pub mod_id: i32,
    pub incompatibility_id: i32,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance 
}

pub struct DependencyCreate {
    pub dependency_id: i32,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance
}

pub struct IncompatibilityCreate {
    pub incompatibility_id: i32,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance
}

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy)]
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

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy)]
#[sqlx(type_name = "dependency_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DependencyImportance {
    Suggested,
    Recommended,
    Required
}

#[derive(sqlx::Type, Debug, Serialize, Clone, Copy, Deserialize)]
#[sqlx(type_name = "dependency_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum IncompatibilityImportance {
    Breaking,
    Conflicting
}

impl Dependency {
    pub async fn create_for_mod_version(id: i32, deps: Vec<DependencyCreate>, pool: &mut PgConnection) -> Result<(), ApiError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO dependencies (dependent_id, dependency_id, compare, importance) VALUES ");
        let mut index = 0;
        for i in &deps {
            let mut separated = builder.separated(", ");
            separated.push_unseparated("(");
            separated.push_bind(id);
            separated.push_bind(i.dependency_id);
            separated.push_bind(i.compare);
            separated.push_bind(i.importance);
            separated.push_unseparated(")");
            if index != deps.len() - 1 {
                separated.push_unseparated(", ");
            }
            index += 1;
        }
        log::info!("{}", builder.sql());

        let result = builder.build().execute(&mut *pool).await;
        if result.is_err() {
            log::error!("{:?}", result.err().unwrap());
            return Err(ApiError::DbError);
        }

        Ok(())
    }
}

impl Incompatibility {
    pub async fn create_for_mod_version(id: i32, deps: Vec<IncompatibilityCreate>, pool: &mut PgConnection) -> Result<(), ApiError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO incompatibilities (mod_id, incompatible_id, compare, importance) VALUES ");
        let mut index = 0;
        for i in &deps {
            let mut separated = builder.separated(", ");
            separated.push_unseparated("(");
            separated.push_bind(id);
            separated.push_bind(i.incompatibility_id);
            separated.push_bind(i.compare);
            separated.push_bind(i.importance);
            log::info!("{}", index);
            separated.push_unseparated(")");
            if index != deps.len() - 1 {
                separated.push_unseparated(", ");
            }
            index += 1;
        }

        let result = builder.build().execute(&mut *pool).await;
        if result.is_err() {
            log::error!("{:?}", result.err().unwrap());
            return Err(ApiError::DbError);
        }

        Ok(())
    }
}