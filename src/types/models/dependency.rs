use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use crate::database::DatabaseError;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use super::mod_gd_version::{GDVersionEnum, VerPlatform};

#[derive(sqlx::FromRow, Clone)]
pub struct Dependency {}

pub struct DependencyCreate {
    pub dependency_id: String,
    pub version: String,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance,
    pub platforms: Option<HashSet<VerPlatform>>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ResponseDependency {
    pub mod_id: String,
    pub version: String,
    pub importance: DependencyImportance,
    pub platforms: HashSet<VerPlatform>,
}

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct FetchedDependency {
    pub mod_version_id: i32,
    pub version: String,
    pub dependency_id: String,
    pub compare: ModVersionCompare,
    pub importance: DependencyImportance,
    pub windows: bool,
    pub mac_intel: bool,
    pub mac_arm: bool,
    pub android32: bool,
    pub android64: bool,
    pub ios: bool,
}

impl FetchedDependency {
    pub fn into_response(self) -> ResponseDependency {
        let platforms = self.get_platform_hashset();

        ResponseDependency {
            mod_id: self.dependency_id,
            version: {
                if self.version == "*" {
                    "*".to_string()
                } else {
                    format!("{}{}", self.compare, self.version)
                }
            },
            importance: self.importance,
            platforms,
        }
    }
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
            platforms: self.get_platform_hashset(),
        }
    }

    fn get_platform_hashset(&self) -> HashSet<VerPlatform> {
        let mut platforms = HashSet::with_capacity(6);
        if self.windows {
            platforms.insert(VerPlatform::Win);
        }
        if self.mac_intel {
            platforms.insert(VerPlatform::MacIntel);
        }
        if self.mac_arm {
            platforms.insert(VerPlatform::MacArm);
        }
        if self.android32 {
            platforms.insert(VerPlatform::Android32);
        }
        if self.android64 {
            platforms.insert(VerPlatform::Android64);
        }
        if self.ios {
            platforms.insert(VerPlatform::Ios);
        }

        platforms
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

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, Default)]
#[sqlx(type_name = "dependency_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DependencyImportance {
    Suggested,
    Recommended,
    #[default]
    Required,
}

#[derive(sqlx::FromRow)]
struct QueryResult {
    start_node: i32,
    dependency_vid: i32,
    dependency_version: String,
    dependency: String,
    importance: DependencyImportance,
    windows: bool,
    mac_intel: bool,
    mac_arm: bool,
    android32: bool,
    android64: bool,
    ios: bool,
}

impl Dependency {
    pub async fn get_for_mod_versions(
        ids: &Vec<i32>,
        platform: Option<VerPlatform>,
        gd: Option<GDVersionEnum>,
        geode: Option<&semver::Version>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, Vec<FetchedDependency>>, DatabaseError> {
        // Fellow developer, I am sorry for what you're about to see :)
        // I present to you the ugly monster of the Geode index
        // The *GigaQuery™*

        let geode_pre = geode.and_then(|x| {
            let pre = x.pre.to_string();
            (!pre.is_empty()).then_some(pre)
        });

        let result: Vec<QueryResult> = sqlx::query_as(
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
                        dp.windows as windows,
                        dp.mac_intel as mac_intel,
                        dp.mac_arm as mac_arm,
                        dp.android32 as android32,
                        dp.android64 as android64,
                        dp.ios as ios,
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
                    AND ($4 IS NULL OR $4 = dpcy_version.geode_major)
                    AND ($5 IS NULL OR $5 >= dpcy_version.geode_minor)
                    AND (
                        ($7 IS NULL AND dpcy_version.geode_meta NOT ILIKE 'alpha%')
                        OR (
                            $7 ILIKE 'alpha%'
                            AND $5 = dpcy_version.geode_minor
                            AND $6 = dpcy_version.geode_patch
                            AND $7 = dpcy_version.geode_meta
                        )
                        OR (
                            dpcy_version.geode_meta IS NULL
                            OR $5 > dpcy_version.geode_minor
                            OR $6 > dpcy_version.geode_patch
                            OR (dpcy_version.geode_meta NOT ILIKE 'alpha%' AND $7 >= dpcy_version.geode_meta)
                        )
                    )
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
                        dp2.windows as windows,
                        dp2.mac_intel as mac_intel,
                        dp2.mac_arm as mac_arm,
                        dp2.android32 as android32,
                        dp2.android64 as android64,
                        dp2.ios as ios,
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
                    AND ($4 IS NULL OR $4 = dpcy_version2.geode_major)
                    AND ($5 IS NULL OR $5 >= dpcy_version2.geode_minor)
                    AND (
                        ($7 IS NULL AND dpcy_version2.geode_meta NOT ILIKE 'alpha%')
                        OR (
                            $7 ILIKE 'alpha%'
                            AND $5 = dpcy_version2.geode_minor
                            AND $6 = dpcy_version2.geode_patch
                            AND $7 = dpcy_version2.geode_meta
                        )
                        OR (
                            dpcy_version2.geode_meta IS NULL
                            OR $5 > dpcy_version2.geode_minor
                            OR $6 > dpcy_version2.geode_patch
                            OR (dpcy_version2.geode_meta NOT ILIKE 'alpha%' AND $7 >= dpcy_version2.geode_meta)
                        )
                    )
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
        .bind(geode.map(|x| i32::try_from(x.major).unwrap_or_default()))
        .bind(geode.map(|x| i32::try_from(x.minor).unwrap_or_default()))
        .bind(geode.map(|x| i32::try_from(x.patch).unwrap_or_default()))
        .bind(geode_pre)
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|x| log::error!("Failed to fetch dependencies: {x}"))?;

        let mut ret: HashMap<i32, Vec<FetchedDependency>> = HashMap::new();
        for i in result {
            if let Some(p) = &platform {
                if should_skip_row(p, &i) {
                    continue;
                }
            }

            ret.entry(i.start_node)
                .or_default()
                .push(FetchedDependency {
                    mod_version_id: i.dependency_vid,
                    version: i.dependency_version.clone(),
                    dependency_id: i.dependency,
                    compare: ModVersionCompare::Exact,
                    importance: i.importance,
                    windows: i.windows,
                    mac_intel: i.mac_intel,
                    mac_arm: i.mac_arm,
                    android32: i.android32,
                    android64: i.android64,
                    ios: i.ios,
                });
        }
        Ok(ret)
    }
}

fn should_skip_row(platform: &VerPlatform, row: &QueryResult) -> bool {
    match platform {
        VerPlatform::Win => row.windows,
        VerPlatform::Android => row.android32 || row.android64,
        VerPlatform::Android32 => row.android32,
        VerPlatform::Android64 => row.android64,
        VerPlatform::Ios => row.ios,
        VerPlatform::Mac => row.mac_intel || row.mac_arm,
        VerPlatform::MacArm => row.mac_arm,
        VerPlatform::MacIntel => row.mac_intel,
    }
}
