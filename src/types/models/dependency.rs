use std::{collections::HashMap, fmt::Display};

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, QueryBuilder};

use crate::types::api::ApiError;

use super::mod_gd_version::{GDVersionEnum, VerPlatform};

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
        platform: Option<VerPlatform>,
        gd: Option<GDVersionEnum>,
        geode: Option<&semver::Version>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, Vec<FetchedDependency>>, ApiError> {
        // Fellow developer, I am sorry for what you're about to see :)

        #[derive(sqlx::FromRow)]
        struct QueryResult {
            start_node: i32,
            dependency_vid: i32,
            dependency_version: String,
            dependency: String,
            importance: DependencyImportance,
        }

        let q = sqlx::query_as::<Postgres, QueryResult>(
            r#"
            WITH RECURSIVE dep_tree AS (
                SELECT * FROM (
                    SELECT 
                        m.id AS id,
                        mv.id AS mod_version_id,
                        mv.name AS name,
                        mv.version AS version,
                        dp.compare as compare,
                        dp.importance as importance,
                        dp.version AS needs_version,
                        dp.dependency_id AS dependency,
                        dpcy_version.id AS dependency_vid,
                        dpcy_version.name AS depedency_name,
                        dpcy_version.version AS dependency_version,
                        mv.id AS start_node,
                        ROW_NUMBER() OVER(
                            PARTITION BY dp.dependency_id, mv.id 
                            ORDER BY dpcy_version.id DESC, mv.id DESC
                        ) rn 
                    FROM mod_versions mv
                    INNER JOIN mods m ON mv.mod_id = m.id
                    INNER JOIN dependencies dp ON dp.dependent_id = mv.id
                    INNER JOIN mods dpcy ON dp.dependency_id = dpcy.id
                    INNER JOIN mod_versions dpcy_version ON dpcy_version.mod_id = dpcy.id
                    INNER JOIN mod_gd_versions dpcy_mgv ON dpcy_version.id = dpcy_mgv.mod_id
                    INNER JOIN mod_version_statuses dpcy_status ON dpcy_version.status_id = dpcy_status.id
                    WHERE dpcy_status.status = 'accepted'
                    AND mv.id = ANY($1)
                    AND ($2 IS NULL OR dpcy_mgv.gd = $2 OR dpcy_mgv.gd = '*')
                    AND ($3 IS NULL OR dpcy_mgv.platform = $3)
                    AND ($4 IS NULL OR (
                        CASE
                            WHEN SPLIT_PART($4, '-', 2) ILIKE 'alpha%' THEN $4 = dpcy_version.geode
                            ELSE SPLIT_PART($4, '.', 1) = SPLIT_PART(dpcy_version.geode, '.', 1)
                                AND SPLIT_PART(dpcy_version.geode, '-', 2) NOT LIKE 'alpha%'
                                AND SPLIT_PART(dpcy_version.geode, '.', 2) <= SPLIT_PART($4, '.', 2)
                                AND (
                                    SPLIT_PART(dpcy_version.geode, '-', 2) = '' 
                                    OR SPLIT_PART(dpcy_version.geode, '-', 2) <= SPLIT_PART($4, '-', 2)
                                )
                        END
                    ))
                    AND SPLIT_PART(dpcy_version.version, '.', 1) = SPLIT_PART(dp.version, '.', 1)
                    AND CASE
                        WHEN dp.version = '*' THEN true
                        WHEN dp.compare = '<' THEN semver_compare(dpcy_version.version, dp.version) = -1
                        WHEN dp.compare = '>' THEN semver_compare(dpcy_version.version, dp.version) = 1
                        WHEN dp.compare = '<=' THEN semver_compare(dpcy_version.version, dp.version) <= 0
                        WHEN dp.compare = '>=' THEN semver_compare(dpcy_version.version, dp.version) >= 0
                        WHEN dp.compare = '=' THEN semver_compare(dpcy_version.version, dp.version) = 0
                        ELSE false
                    END
                ) as q
                WHERE q.rn = 1
                UNION
                SELECT * FROM (
                    SELECT 
                        m2.id AS id,
                        mv2.id AS mod_version_id,
                        mv2.name AS name,
                        mv2.version AS version,
                        dp2.compare AS needs_compare,
                        dp2.importance as importance,
                        dp2.version AS needs_version,
                        dp2.dependency_id AS dependency,
                        dpcy_version2.id AS dependency_vid,
                        dpcy_version2.name AS depedency_name,
                        dpcy_version2.version AS dependency_version,
                        dt.start_node AS start_node,
                        ROW_NUMBER() OVER(
                            PARTITION BY dp2.dependency_id, mv2.id 
                            ORDER BY dpcy_version2.id DESC, mv2.id DESC
                        ) rn 
                    FROM mod_versions mv2
                    INNER JOIN mods m2 ON mv2.mod_id = m2.id
                    INNER JOIN dependencies dp2 ON dp2.dependent_id = mv2.id
                    INNER JOIN mods dpcy2 ON dp2.dependency_id = dpcy2.id
                    INNER JOIN mod_versions dpcy_version2 ON dpcy_version2.mod_id = dpcy2.id
                    INNER JOIN mod_gd_versions dpcy_mgv2 ON dpcy_version2.id = dpcy_mgv2.mod_id
                    INNER JOIN mod_version_statuses dpcy_status2 ON dpcy_version2.status_id = dpcy_status2.id
                    INNER JOIN dep_tree dt ON dt.dependency_vid = mv2.id
                    WHERE dpcy_status2.status = 'accepted'
                    AND ($2 IS NULL OR dpcy_mgv2.gd = $2 OR dpcy_mgv2.gd = '*')
                    AND ($3 IS NULL OR dpcy_mgv2.platform = $3)
                    AND ($4 IS NULL OR (
                        CASE
                            WHEN SPLIT_PART($4, '-', 2) ILIKE 'alpha%' THEN $4 = dpcy_version2.geode
                            ELSE SPLIT_PART($4, '.', 1) = SPLIT_PART(dpcy_version2.geode, '.', 1) 
                                AND SPLIT_PART(dpcy_version2.geode, '-', 2) NOT LIKE 'alpha%'
                                AND SPLIT_PART(dpcy_version2.geode, '.', 2) <= SPLIT_PART($4, '.', 2)
                                AND (
                                    SPLIT_PART(dpcy_version2.geode, '-', 2) = '' 
                                    OR SPLIT_PART(dpcy_version2.geode, '-', 2) <= SPLIT_PART($4, '-', 2)
                                )
                        END
                    ))
                    AND SPLIT_PART(dpcy_version2.version, '.', 1) = SPLIT_PART(dp2.version, '.', 1)
                    AND CASE
                        WHEN dp2.version = '*' THEN true
                        WHEN dp2.compare = '<' THEN semver_compare(dpcy_version2.version, dp2.version) = -1
                        WHEN dp2.compare = '>' THEN semver_compare(dpcy_version2.version, dp2.version) = 1
                        WHEN dp2.compare = '<=' THEN semver_compare(dpcy_version2.version, dp2.version) <= 0
                        WHEN dp2.compare = '>=' THEN semver_compare(dpcy_version2.version, dp2.version) >= 0
                        WHEN dp2.compare = '=' THEN semver_compare(dpcy_version2.version, dp2.version) = 0
                        ELSE false
                    END
                ) as q2
                WHERE q2.rn = 1
            )
            SELECT * FROM dep_tree;
            "#,
        ).bind(ids)
        .bind(gd)
        .bind(platform)
        .bind(geode.map(|x| x.to_string()));

        let result = match q.fetch_all(&mut *pool).await {
            Ok(d) => d,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        let mut ret: HashMap<i32, Vec<FetchedDependency>> = HashMap::new();
        for i in result {
            ret.entry(i.start_node)
                .or_default()
                .push(FetchedDependency {
                    mod_version_id: i.dependency_vid,
                    version: i.dependency_version.clone(),
                    dependency_id: i.dependency,
                    compare: ModVersionCompare::Exact,
                    importance: i.importance,
                });
        }
        Ok(ret)
    }
}
