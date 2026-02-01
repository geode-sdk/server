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

    pub async fn get_supersedes_for(
        ids: &Vec<String>,
        platform: VerPlatform,
        gd: GDVersionEnum,
        geode: &semver::Version,
        pool: &mut PgConnection,
    ) -> Result<HashMap<String, Replacement>, DatabaseError> {
        let mut ret: HashMap<String, Replacement> = HashMap::new();
        let pre = if geode.pre.is_empty() {
            None
        } else {
            Some(geode.pre.to_string())
        };
        let r = sqlx::query!(
            r#"
            SELECT 
                q.replaced,
                q.replacement,
                q.replacement_version,
                q.replacement_id
            FROM (
                SELECT 
                    replaced.incompatibility_id AS replaced, 
                    replacement.mod_id AS replacement, 
                    replacement.version AS replacement_version,
                    replacement.id AS replacement_id,
                    ROW_NUMBER() OVER(
                        partition by replacement.mod_id 
                        order by replacement.version desc
                    ) rn
                FROM incompatibilities replaced
                INNER JOIN mod_versions replacement ON replacement.id = replaced.mod_id
                INNER JOIN mod_gd_versions replacement_mgv ON replacement.id = replacement_mgv.mod_id
                INNER JOIN mod_version_statuses replacement_status 
                    ON replacement.status_id = replacement_status.id
                WHERE replaced.importance = 'superseded'
                AND replacement_status.status = 'accepted'
                AND replaced.incompatibility_id = ANY($1)
                AND (replacement_mgv.gd = $2 OR replacement_mgv.gd = '*')
                AND replacement_mgv.platform = $3
                AND ($4 = replacement.geode_major)
                AND ($5 >= replacement.geode_minor)
                AND (
                    ($7::text IS NULL AND replacement.geode_meta NOT ILIKE 'alpha%')
                    OR (
                        $7 ILIKE 'alpha%'
                        AND $5 = replacement.geode_minor
                        AND $6 = replacement.geode_patch
                        AND $7 = replacement.geode_meta
                    )
                    OR (
                        replacement.geode_meta IS NULL
                        OR $5 > replacement.geode_minor
                        OR $6 > replacement.geode_patch
                        OR (replacement.geode_meta NOT ILIKE 'alpha%' AND $7 >= replacement.geode_meta)
                    )
                )
                ORDER BY replacement.id DESC, replacement.version DESC
            ) q
            WHERE q.rn = 1
            "#,
            ids,
            gd as GDVersionEnum,
            platform as VerPlatform,
            i32::try_from(geode.major).unwrap_or_default(),
            i32::try_from(geode.minor).unwrap_or_default(),
            i32::try_from(geode.patch).unwrap_or_default(),
            pre
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| log::error!("Failed to fetch supersedes: {e}"))?;

        // Client doesn't actually use those, we might as well not return them yet
        // TODO: enable back when client supports then
        // let ids: Vec<i32> = r.iter().map(|x| x.replacement_id).collect();
        // let deps =
        //     Dependency::get_for_mod_versions(&ids, Some(platform), Some(gd), Some(geode), pool)
        //         .await?;
        // let incompat = Incompatibility::get_for_mod_versions(
        //     &ids,
        //     Some(platform),
        //     Some(gd),
        //     Some(geode),
        //     pool,
        // )
        // .await?;

        for i in r.iter() {
            ret.entry(i.replaced.clone()).or_insert(Replacement {
                id: i.replacement.clone(),
                version: i.replacement_version.clone(),
                replacement_id: i.replacement_id,
                // Should be completed later
                download_link: "".to_string(),
                dependencies: vec![],
                incompatibilities: vec![], // dependencies: deps
                                           //     .get(&i.replacement_id)
                                           //     .cloned()
                                           //     .unwrap_or_default()
                                           //     .into_iter()
                                           //     .map(|x| x.to_response())
                                           //     .collect(),
                                           // incompatibilities: incompat
                                           //     .get(&i.replacement_id)
                                           //     .cloned()
                                           //     .unwrap_or_default()
                                           //     .into_iter()
                                           //     .filter(|x| {
                                           //         x.importance != IncompatibilityImportance::Superseded
                                           //             && x.incompatibility_id != i.replacement
                                           //     })
                                           //     .map(|x| x.to_response())
                                           //     .collect(),
            });
        }
        Ok(ret)
    }
}
