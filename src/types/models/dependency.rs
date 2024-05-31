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

    pub async fn get_for_mod_versions(
        ids: &Vec<i32>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, Vec<FetchedDependency>>, ApiError> {
        // Fellow developer, I am sorry for what you're about to see :)

        let result = match sqlx::query!(
            r#"
            WITH RECURSIVE dep_tree AS (
                SELECT * FROM (
                    SELECT 
                        m.id AS id,
                        mv.id AS mod_version_id,
                        mv.name AS name,
                        mv.version AS version,
                        dp.compare AS "needs_compare: ModVersionCompare",
                        dp.importance as "importance: DependencyImportance",
                        dp.version AS needs_version,
                        dp.dependency_id AS dependency,
                        dpcy_version.id AS dependency_vid,
                        dpcy_version.name AS depedency_name,
                        dpcy_version.version AS dependency_version,
                        mv.id AS start_node,
                        ROW_NUMBER() OVER(
                            PARTITION BY dp.dependency_id, mv.id 
                            ORDER BY dpcy_version.version DESC, mv.version DESC
                        ) rn 
                    FROM mod_versions mv
                    INNER JOIN mods m ON mv.mod_id = m.id
                    INNER JOIN dependencies dp ON dp.dependent_id = mv.id
                    INNER JOIN mods dpcy ON dp.dependency_id = dpcy.id
                    INNER JOIN mod_versions dpcy_version ON dpcy_version.mod_id = dpcy.id
                    INNER JOIN mod_version_statuses dpcy_status ON dpcy_version.status_id = dpcy_status.id
                    WHERE dpcy_status.status = 'accepted'
                    AND mv.id = ANY($1)
                    AND CASE
                        WHEN dp.version = '*' THEN 1
                            WHEN SPLIT_PART(dpcy_version.version, '.', 1) = SPLIT_PART(dp.version, '.', 1) THEN 1
                        ELSE 0
                    END = 1
                    AND CASE
                        WHEN dp.version = '*' THEN 1
                        WHEN dp.compare = '<' AND dpcy_version.version < dp.version THEN 1
                        WHEN dp.compare = '>' AND dpcy_version.version > dp.version THEN 1
                        WHEN dp.compare = '<=' AND dpcy_version.version <= dp.version THEN 1
                        WHEN dp.compare = '>=' AND dpcy_version.version >= dp.version THEN 1
                        WHEN dp.compare = '=' AND dpcy_version.version = dp.version THEN 1
                        ELSE 0
                    END = 1
                ) as q
                WHERE q.rn = 1
                UNION
                SELECT * FROM (
                    SELECT 
                        m2.id AS id,
                        mv2.id AS mod_version_id,
                        mv2.name AS name,
                        mv2.version AS version,
                        dp2.compare AS "needs_compare: ModVersionCompare",
                        dp2.importance as "importance: DependencyImportance",
                        dp2.version AS needs_version,
                        dp2.dependency_id AS dependency,
                        dpcy_version2.id AS dependency_vid,
                        dpcy_version2.name AS depedency_name,
                        dpcy_version2.version AS dependency_version,
                        dt.start_node AS start_node,
                        ROW_NUMBER() OVER(
                            PARTITION BY dp2.dependency_id, mv2.id 
                            ORDER BY dpcy_version2.version DESC, mv2.version DESC
                        ) rn 
                    FROM mod_versions mv2
                    INNER JOIN mods m2 ON mv2.mod_id = m2.id
                    INNER JOIN dependencies dp2 ON dp2.dependent_id = mv2.id
                    INNER JOIN mods dpcy2 ON dp2.dependency_id = dpcy2.id
                    INNER JOIN mod_versions dpcy_version2 ON dpcy_version2.mod_id = dpcy2.id
                    INNER JOIN mod_version_statuses dpcy_status2 ON dpcy_version2.status_id = dpcy_status2.id
                    INNER JOIN dep_tree dt ON dt.dependency_vid = mv2.id
                    WHERE dpcy_status2.status = 'accepted'
                    AND CASE
                        WHEN dp2.version = '*' THEN 1
                            WHEN SPLIT_PART(dpcy_version2.version, '.', 1) = SPLIT_PART(dp2.version, '.', 1) THEN 1
                        ELSE 0
                    END = 1
                    AND CASE
                        WHEN dp2.version = '*' THEN 1
                        WHEN dp2.compare = '<' AND dpcy_version2.version < dp2.version THEN 1
                        WHEN dp2.compare = '>' AND dpcy_version2.version > dp2.version THEN 1
                        WHEN dp2.compare = '<=' AND dpcy_version2.version <= dp2.version THEN 1
                        WHEN dp2.compare = '>=' AND dpcy_version2.version >= dp2.version THEN 1
                        WHEN dp2.compare = '=' AND dpcy_version2.version = dp2.version THEN 1
                        ELSE 0
                    END = 1
                ) as q2
                WHERE q2.rn = 1
            )
            SELECT * FROM dep_tree;
            "#,
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
            ret.entry(i.start_node.unwrap())
                .or_default()
                .push(FetchedDependency {
                    mod_version_id: i.dependency_vid.unwrap(),
                    version: i.dependency_version.clone().unwrap(),
                    dependency_id: i.dependency.clone().unwrap(),
                    compare: ModVersionCompare::Exact,
                    importance: i.importance.unwrap(),
                });
        }
        Ok(ret)
    }
}
