use std::collections::HashMap;

use super::{
    dependency::ResponseDependency,
    mod_gd_version::{GDVersionEnum, VerPlatform},
};
use crate::database::DatabaseError;
use crate::types::models::dependency::ModVersionCompare;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres};

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct FetchedIncompatibility {
    pub mod_id: i32,
    pub version: String,
    pub incompatibility_id: String,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance,
}

pub struct IncompatibilityCreate {
    pub incompatibility_id: String,
    pub version: String,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance,
}

#[derive(sqlx::FromRow)]
pub struct Incompatibility {}

#[derive(Debug, Serialize, Clone)]
pub struct Replacement {
    pub id: String,
    pub version: String,
    #[serde(skip_serializing)]
    pub replacement_id: i32,
    pub download_link: String,
    pub dependencies: Vec<ResponseDependency>,
    pub incompatibilities: Vec<ResponseIncompatibility>,
}

#[derive(sqlx::Type, Debug, Serialize, Clone, Copy, Deserialize, PartialEq, Default)]
#[sqlx(type_name = "incompatibility_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum IncompatibilityImportance {
    #[default]
    Breaking,
    Conflicting,
    Superseded,
}

#[derive(Serialize, Debug, Clone)]
pub struct ResponseIncompatibility {
    pub mod_id: String,
    pub version: String,
    pub importance: IncompatibilityImportance,
    pub breaking: bool,
}

impl FetchedIncompatibility {
    pub fn into_response(self) -> ResponseIncompatibility {
        ResponseIncompatibility {
            mod_id: self.incompatibility_id,
            version: {
                if self.version == "*" {
                    "*".to_string()
                } else {
                    format!("{}{}", self.compare, self.version)
                }
            },
            importance: self.importance,
            breaking: self.importance == IncompatibilityImportance::Breaking,
        }
    }

    pub fn to_response(&self) -> ResponseIncompatibility {
        ResponseIncompatibility {
            mod_id: self.incompatibility_id.clone(),
            version: {
                if self.version == "*" {
                    "*".to_string()
                } else {
                    format!("{}{}", self.compare, self.version)
                }
            },
            importance: self.importance,
            breaking: self.importance == IncompatibilityImportance::Breaking,
        }
    }
}

impl Incompatibility {
    pub async fn get_for_mod_version(
        id: i32,
        pool: &mut PgConnection,
    ) -> Result<Vec<FetchedIncompatibility>, DatabaseError> {
        sqlx::query_as!(
            FetchedIncompatibility,
            r#"SELECT icp.compare as "compare: _",
            icp.importance as "importance: _",
            icp.incompatibility_id, icp.mod_id, icp.version FROM incompatibilities icp
            INNER JOIN mod_versions mv ON mv.id = icp.mod_id
            WHERE mv.id = $1"#,
            id
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| log::error!("Failed to fetch incompatibilities for mod_version {id}: {e}"))
        .map_err(|e| e.into())
    }

    pub async fn get_for_mod_versions(
        ids: &Vec<i32>,
        platform: Option<VerPlatform>,
        gd: Option<GDVersionEnum>,
        geode: Option<&semver::Version>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, Vec<FetchedIncompatibility>>, DatabaseError> {
        let geode_pre = geode.and_then(|x| {
            if x.pre.is_empty() {
                None
            } else {
                Some(x.pre.to_string())
            }
        });

        let q = sqlx::query_as::<Postgres, FetchedIncompatibility>(
            r#"SELECT icp.compare,
            icp.importance,
            icp.incompatibility_id, icp.mod_id, icp.version FROM incompatibilities icp
            INNER JOIN mod_versions mv ON mv.id = icp.mod_id
            INNER JOIN mod_gd_versions mgv ON mv.id = mgv.mod_id
            WHERE mv.id = ANY($1)
            AND ($2 IS NULL OR mgv.gd = $2)
            AND ($3 IS NULL OR mgv.platform = $3)
            AND ($4 IS NULL OR $4 = mv.geode_major)
            AND ($5 IS NULL OR $5 >= mv.geode_minor)
            AND (
                ($7 IS NULL AND mv.geode_meta NOT ILIKE 'alpha%')
                OR (
                    $7 ILIKE 'alpha%'
                    AND $5 = mv.geode_minor
                    AND $6 = mv.geode_patch
                    AND $7 = mv.geode_meta
                )
                OR (
                    mv.geode_meta IS NULL
                    OR $5 > mv.geode_minor
                    OR $6 > mv.geode_patch
                    OR (mv.geode_meta NOT ILIKE 'alpha%' AND $7 >= mv.geode_meta)
                )
            )
            "#,
        )
        .bind(ids)
        .bind(gd)
        .bind(platform)
        .bind(geode.map(|x| i64::try_from(x.major).ok()))
        .bind(geode.map(|x| i64::try_from(x.minor).ok()))
        .bind(geode.map(|x| i64::try_from(x.patch).ok()))
        .bind(geode_pre);

        let result = q.fetch_all(&mut *pool).await.inspect_err(|e| {
            log::error!("Failed to fetch incompatibilities for mod_versions: {e}")
        })?;

        let mut ret: HashMap<i32, Vec<FetchedIncompatibility>> = HashMap::new();

        for i in result {
            ret.entry(i.mod_id).or_default().push(i);
        }

        Ok(ret)
    }
}
