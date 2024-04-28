use std::{collections::HashMap, fmt::Display};

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, QueryBuilder};

use crate::types::api::ApiError;

#[derive(sqlx::FromRow, Clone)]
pub struct Dependency {
    pub dependent_id: i32,
    pub dependency_id: String,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance,
}

pub struct DependencyCreate {
    pub dependency_id: String,
    pub version: String,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance,
}

#[derive(Serialize, Debug, Clone)]
pub struct ResponseDependency {
    pub mod_id: String,
    pub version: String,
    pub importance: DependencyImportance,
}

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct FetchedDependency {
    pub mod_version_id: i32,
    pub version: String,
    pub dependency_id: String,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance,
}

impl FetchedDependency {
    pub fn to_response(&self) -> ResponseDependency {
        ResponseDependency {
            mod_id: self.dependency_id.clone(),
            version: {
                if self.version == "*" {
                    "*".to_string()
                } else {
                    format!("{}{}", self.compare, self.version)
                }
            },
            importance: self.importance,
        }
    }
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
    #[serde(rename = "<=")]
    #[sqlx(rename = "<=")]
    LessEq,
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
    Required,
}

impl Dependency {
    pub async fn create_for_mod_version(
        id: i32,
        deps: Vec<DependencyCreate>,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO dependencies (dependent_id, dependency_id, version, compare, importance) VALUES ",
        );
        for (index, i) in deps.iter().enumerate() {
            let mut separated = builder.separated(", ");
            separated.push_unseparated("(");
            separated.push_bind(id);
            separated.push_bind(&i.dependency_id);
            separated.push_bind(&i.version);
            separated.push_bind(i.compare);
            separated.push_bind(i.importance);
            separated.push_unseparated(")");
            if index != deps.len() - 1 {
                separated.push_unseparated(", ");
            }
        }

        let result = builder.build().execute(&mut *pool).await;
        if result.is_err() {
            log::error!("{:?}", result.err().unwrap());
            return Err(ApiError::DbError);
        }

        Ok(())
    }

    pub async fn get_for_mod_version(
        id: i32,
        pool: &mut PgConnection,
    ) -> Result<Vec<FetchedDependency>, ApiError> {
        match sqlx::query_as!(
            FetchedDependency,
            r#"SELECT dp.dependent_id as mod_version_id, dp.dependency_id, 
            dp.version, dp.compare AS "compare: _", dp.importance AS "importance: _" 
            FROM dependencies dp
            WHERE dp.dependent_id = $1"#,
            id
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(d) => Ok(d),
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
        }
    }

    pub async fn get_for_mod_versions(
        ids: &Vec<i32>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, Vec<FetchedDependency>>, ApiError> {
        let result = match sqlx::query_as!(FetchedDependency,
            r#"SELECT dp.dependent_id as mod_version_id, dp.dependency_id, dp.version, dp.compare AS "compare: _", dp.importance AS "importance: _" FROM dependencies dp
            WHERE dp.dependent_id = ANY($1)"#,
            &ids
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(d) => d,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        let mut ret: HashMap<i32, Vec<FetchedDependency>> = HashMap::new();
        for i in result {
            ret.entry(i.mod_version_id).or_default().push(i);
        }
        Ok(ret)
    }
}
