use std::fmt::Display;

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, QueryBuilder};

use crate::types::api::ApiError;

use super::mod_version::ModVersion;

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
        ver: &ModVersion,
        pool: &mut PgConnection,
    ) -> Result<Vec<FetchedDependency>, ApiError> {
        match sqlx::query_as!(FetchedDependency,
            r#"SELECT dp.dependent_id as mod_version_id, dp.dependency_id, dp.version, dp.compare AS "compare: _", dp.importance AS "importance: _" FROM dependencies dp
            WHERE dp.dependent_id = $1"#,
            ver.id
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
}

// This is a graveyard of my hopes and dreams wasted on trying to resolve a dependency graph serverside

// async fn merge_version_cache(
//     existing: &mut HashMap<String, Vec<FetchedModVersionDep>>,
//     mod_ids_to_fetch: Vec<String>,
//     pool: &mut PgConnection,
// ) -> Result<(), ApiError> {
//     struct Fetched {
//         mod_version_id: i32,
//         version: String,
//     }
//     let versions = match sqlx::query_as!(Fetched,
//         "SELECT id as mod_version_id, version FROM mod_versions WHERE mod_id = ANY($1) AND validated = true",
//         &mod_ids_to_fetch
//     ).fetch_all(&mut *pool).await {
//         Ok(d) => d,
//         Err(e) => {
//             log::error!("{}", e);
//             return Err(ApiError::DbError);
//         }
//     };

//     let versions = versions
//         .iter()
//         .map(|x| FetchedModVersionDep {
//             mod_version_id: x.mod_version_id,
//             version: semver::Version::parse(&x.version).unwrap(),
//         })
//         .collect::<Vec<FetchedModVersionDep>>();

//     let mut fetched: HashMap<String, Vec<FetchedModVersionDep>> = HashMap::new();
//     for i in versions {
//         if !fetched.contains_key(&i.mod_version_id.to_string()) {
//             fetched.insert(i.mod_version_id.to_string(), vec![]);
//         }
//         fetched
//             .get_mut(&i.mod_version_id.to_string())
//             .unwrap()
//             .push(i);
//     }

//     for (id, versions) in fetched {
//         if existing.contains_key(&id) {
//             continue;
//         }
//         existing.insert(id, versions);
//     }
//     Ok(())
// }

// fn get_matches_for_dependencies(
//     deps: Vec<FetchedDependency>,
//     cached: &HashMap<String, Vec<FetchedModVersionDep>>,
// ) -> Vec<FetchedModVersionDep> {
//     let mut ret: Vec<FetchedModVersionDep> = vec![];
//     for i in deps {
//         let versions = match cached.get(&i.dependency_id) {
//             Some(v) => v,
//             None => continue,
//         };
//         let dependency_parsed = semver::Version::parse(&i.version).unwrap();
//         let mut max: Option<Version>;
//         let mut best: Option<FetchedModVersionDep>;

//         let valids: Vec<FetchedModVersionDep> = match i.compare {
//             ModVersionCompare::Exact => versions
//                 .iter()
//                 .filter(|x| x.version == dependency_parsed)
//                 .map(|x| x.clone())
//                 .collect(),
//             ModVersionCompare::More => versions
//                 .iter()
//                 .filter(|x| x.version > dependency_parsed)
//                 .map(|x| x.clone())
//                 .collect(),
//             ModVersionCompare::MoreEq => versions
//                 .iter()
//                 .filter(|x| x.version >= dependency_parsed)
//                 .map(|x| x.clone())
//                 .collect(),
//             ModVersionCompare::Less => versions
//                 .iter()
//                 .filter(|x| x.version < dependency_parsed)
//                 .map(|x| x.clone())
//                 .collect(),
//             ModVersionCompare::LessEq => versions
//                 .iter()
//                 .filter(|x| x.version <= dependency_parsed)
//                 .map(|x| x.clone())
//                 .collect(),
//         };
//         for i in valids {
//             if let Some(m) = max {
//                 if i.version > m {
//                     max = Some(i.version);
//                     best = Some(i.clone());
//                 }
//             } else {
//                 max = Some(i.version);
//                 best = Some(i.clone());
//             }
//         }
//         if let Some(b) = best {
//             ret.push(b);
//         }
//     }
//     ret
// }
