use std::fmt::Display;

use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Postgres, PgConnection};

use crate::types::api::ApiError;

#[derive(sqlx::FromRow, Clone, Copy)]
pub struct Dependency {
    pub dependent_id: i32,
    pub dependency_id: i32,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance
}

pub struct DependencyCreate {
    pub dependency_id: i32,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance
}


#[derive(Serialize, Debug, Clone)]
pub struct ResponseDependency {
    pub mod_id: String,
    pub version: String,
    pub importance: DependencyImportance
}

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct FetchedDependency {
    pub mod_id: String,
    pub version: String,
    pub dependency_id: i32,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance
}


#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
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

impl Display for ModVersionCompare {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact => write!(f, "="),
            Self::Less => write!(f, "<"),
            Self::More => write!(f, ">"),
            Self::LessEq => write!(f, "<="),
            Self::MoreEq => write!(f, ">="),
        } 
    }
}

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy)]
#[sqlx(type_name = "dependency_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DependencyImportance {
    Suggested,
    Recommended,
    Required
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

        let result = builder.build().execute(&mut *pool).await;
        if result.is_err() {
            log::error!("{:?}", result.err().unwrap());
            return Err(ApiError::DbError);
        }

        Ok(())
    }

    pub async fn get_for_mod_version(id: i32, pool: &mut PgConnection) -> Result<Vec<FetchedDependency>, ApiError> {
        let mut ret: Vec<FetchedDependency> = vec![];
        let mut modifiable_ids = vec![id];
        loop {
            if modifiable_ids.len() == 0 {
                break;
            }
            let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT dp.dependency_id, dp.compare, dp.importance, mv.version, mv.mod_id FROM dependencies dp
            INNER JOIN mod_versions mv ON dp.dependency_id = mv.id 
            WHERE dp.dependent_id IN (");
            let mut separated = builder.separated(",");
            let copy = ret.clone();
            for i in &modifiable_ids {
                separated.push_bind(i);
            }
            separated.push_unseparated(")");
            let result = builder.build_query_as::<FetchedDependency>()
                .fetch_all(&mut *pool)
                .await;
            if result.is_err() {
                log::info!("{}", result.err().unwrap());
                return Err(ApiError::DbError);
            }
            let result = result.unwrap();
            if result.is_empty() {
                break;
            }
            modifiable_ids.clear();
            for i in result {
                let doubled = copy.iter().find(|x| {x.dependency_id == i.dependency_id});
                if doubled.is_none() {
                    modifiable_ids.push(i.dependency_id);
                    ret.push(i);
                    continue;
                }
                // this is a sketchy bit
                let doubled = doubled.unwrap();
                if should_add(doubled, &i) {
                    modifiable_ids.push(i.dependency_id);
                    ret.push(i);
                }
            }
        }
        Ok(ret)
    }
}


fn should_add(old: &FetchedDependency, new: &FetchedDependency) -> bool {
    old.compare != new.compare || old.version != new.version
}