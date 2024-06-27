use std::collections::HashMap;

use crate::types::api::ApiError;
use crate::types::models::dependency::ModVersionCompare;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, QueryBuilder};

use super::{
    dependency::ResponseDependency,
    mod_gd_version::{GDVersionEnum, VerPlatform},
};

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
pub struct Incompatibility {
    pub mod_id: i32,
    pub incompatibility_id: String,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance,
}

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

#[derive(sqlx::Type, Debug, Serialize, Clone, Copy, Deserialize, PartialEq)]
#[sqlx(type_name = "incompatibility_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum IncompatibilityImportance {
    Breaking,
    Conflicting,
    Superseded,
}

#[derive(Serialize, Debug, Clone)]
pub struct ResponseIncompatibility {
    pub mod_id: String,
    pub version: String,
    pub importance: IncompatibilityImportance,
}

impl FetchedIncompatibility {
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
        }
    }
}

impl Incompatibility {
    pub async fn create_for_mod_version(
        id: i32,
        incompats: Vec<IncompatibilityCreate>,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO incompatibilities (mod_id, incompatibility_id, version, compare, importance) VALUES ",
        );
        for (index, i) in incompats.iter().enumerate() {
            let mut separated = builder.separated(", ");
            separated.push_unseparated("(");
            separated.push_bind(id);
            separated.push_bind(&i.incompatibility_id);
            separated.push_bind(&i.version);
            separated.push_bind(i.compare);
            separated.push_bind(i.importance);
            separated.push_unseparated(")");
            if index != incompats.len() - 1 {
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
    ) -> Result<Vec<FetchedIncompatibility>, ApiError> {
        match sqlx::query_as!(
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
        platform: Option<VerPlatform>,
        gd: Option<GDVersionEnum>,
        geode: Option<&semver::Version>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, Vec<FetchedIncompatibility>>, ApiError> {
        let q = sqlx::query_as::<Postgres, FetchedIncompatibility>(
            r#"SELECT icp.compare,
            icp.importance,
            icp.incompatibility_id, icp.mod_id, icp.version FROM incompatibilities icp
            INNER JOIN mod_versions mv ON mv.id = icp.mod_id
            INNER JOIN mod_gd_versions mgv ON mv.id = mgv.mod_id
            WHERE mv.id = ANY($1)
            AND ($2 IS NULL OR mgv.gd = $2)
            AND ($3 IS NULL OR mgv.platform = $3)
            AND ($4 IS NULL OR CASE
                WHEN SPLIT_PART($4, '-', 2) ILIKE 'alpha%' THEN $4 = mv.geode
                ELSE SPLIT_PART($4, '.', 1) = SPLIT_PART(mv.geode, '.', 1)
                    AND SPLIT_PART(mv.geode, '.', 2) <= SPLIT_PART($4, '.', 2)
                    AND (
                        SPLIT_PART(mv.geode, '-', 2) = '' 
                        OR SPLIT_PART(mv.geode, '-', 2) >= SPLIT_PART($4, '-', 2)
                    )
            END)
            "#,
        )
        .bind(ids)
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
    ) -> Result<HashMap<String, Replacement>, ApiError> {
        let mut ret: HashMap<String, Replacement> = HashMap::new();
        let r = match sqlx::query!(
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
                AND CASE
                    WHEN SPLIT_PART($4, '-', 2) ILIKE 'alpha%' THEN $4 = replacement.geode
                    ELSE SPLIT_PART($4, '.', 1) = SPLIT_PART(replacement.geode, '.', 1)
                        AND semver_compare(replacement.geode, $4) >= 0
                END
                ORDER BY replacement.id DESC, replacement.version DESC
            ) q
            WHERE q.rn = 1
            "#,
            ids,
            gd as GDVersionEnum,
            platform as VerPlatform,
            geode.to_string()
        )
        .fetch_all(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("Failed to fetch supersedes. ERR: {}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

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
