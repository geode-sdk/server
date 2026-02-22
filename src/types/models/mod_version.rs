use super::{
    dependency::{Dependency, ModVersionCompare, ResponseDependency},
    developer::ModDeveloper,
    incompatibility::{Incompatibility, ResponseIncompatibility},
    mod_gd_version::{DetailedGDVersion, GDVersionEnum, ModGDVersion, VerPlatform},
    mod_version_status::ModVersionStatusEnum,
    tag::Tag,
};
use chrono::serde::ts_seconds_option;
use crate::database::DatabaseError;
use crate::database::repository::developers;
use crate::types::api::{PaginatedData, create_download_link};
use semver::Version;
use serde::Serialize;
use sqlx::{
    PgConnection, Postgres, QueryBuilder,
    types::chrono::{DateTime, Utc},
};
use std::collections::HashMap;

#[derive(Serialize, Debug, sqlx::FromRow, Clone)]
pub struct ModVersion {
    #[serde(skip_serializing)]
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub download_link: String,
    pub hash: String,
    pub geode: String,
    pub download_count: i32,
    pub early_load: bool,
    pub requires_patching: bool,
    pub api: bool,
    pub mod_id: String,
    pub gd: DetailedGDVersion,
    pub status: ModVersionStatusEnum,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<ResponseDependency>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incompatibilities: Option<Vec<ResponseIncompatibility>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developers: Option<Vec<ModDeveloper>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(with = "ts_seconds_option")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(with = "ts_seconds_option")]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Admin/developer only - Reason given to status
    pub info: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Admin/developer only - Direct download to mod
    pub direct_download_link: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ModVersionGetOne {
    id: i32,
    name: String,
    description: Option<String>,
    version: String,
    download_link: String,
    download_count: i32,
    hash: String,
    geode: String,
    early_load: bool,
    requires_patching: bool,
    api: bool,
    mod_id: String,
    status: ModVersionStatusEnum,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    info: Option<String>,
}

pub struct IndexQuery {
    pub mod_id: String,
    pub page: i64,
    pub per_page: i64,
    pub gd: Option<GDVersionEnum>,
    pub compare: Option<(semver::Version, ModVersionCompare)>,
    pub platforms: Vec<VerPlatform>,
    pub status: ModVersionStatusEnum,
}

impl ModVersionGetOne {
    pub fn into_mod_version(self) -> ModVersion {
        ModVersion {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            download_link: self.download_link.clone(),
            hash: self.hash.clone(),
            geode: self.geode.clone(),
            early_load: self.early_load,
            requires_patching: self.requires_patching,
            download_count: self.download_count,
            api: self.api,
            mod_id: self.mod_id.clone(),
            status: self.status,
            gd: DetailedGDVersion {
                win: None,
                android: None,
                mac_arm: None,
                mac_intel: None,
                mac: None,
                ios: None,
                android32: None,
                android64: None,
            },
            developers: None,
            tags: None,
            dependencies: None,
            incompatibilities: None,
            info: self.info,
            direct_download_link: None,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl ModVersion {
    fn modify_download_link(&mut self, app_url: &str) {
        self.download_link = create_download_link(app_url, &self.mod_id, &self.version)
    }

    pub fn modify_metadata(&mut self, app_url: &str, keep_information: bool) {
        if keep_information {
            self.direct_download_link = Some(self.download_link.clone());
        } else {
            self.direct_download_link = None;
            self.info = None;
        }

        self.modify_download_link(app_url)
    }

    pub async fn get_index(
        query: IndexQuery,
        pool: &mut PgConnection,
    ) -> Result<PaginatedData<ModVersion>, DatabaseError> {
        let limit = query.per_page;
        let offset = (query.page - 1) * query.per_page;

        let mut q: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT mv.id, mv.name, mv.description, mv.version,
            mv.download_link, mv.download_count, mv.hash,
            format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as geode,
            mv.early_load, mv.requires_patching, mv.api, mv.mod_id, mvs.status, mv.created_at, mv.updated_at
            FROM mod_versions mv
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
            "#,
        );
        let mut counter_q: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT COUNT(DISTINCT mv.id)
            FROM mod_versions mv
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
            "#,
        );
        let sql = "WHERE mv.mod_id = ";
        q.push(sql);
        counter_q.push(sql);
        q.push_bind(&query.mod_id);
        counter_q.push_bind(&query.mod_id);
        let sql = " AND mvs.status = ";
        q.push(sql);
        counter_q.push(sql);
        q.push_bind(query.status);
        counter_q.push_bind(query.status);
        q.push(" ");
        counter_q.push(" ");
        if let Some(gd) = query.gd {
            let sql = "AND (mgv.gd = ";
            q.push(sql);
            counter_q.push(sql);
            q.push_bind(gd);
            counter_q.push_bind(gd);
            let sql = " OR mgv.gd = ";
            q.push(sql);
            counter_q.push(sql);
            q.push_bind(GDVersionEnum::All);
            counter_q.push_bind(GDVersionEnum::All);
            q.push(" ");
            counter_q.push(" ");
        }
        if !query.platforms.is_empty() {
            let sql = "AND mgv.platform IN (";
            q.push(sql);
            counter_q.push(sql);
            let mut separated = q.separated(", ");
            let mut counter_separated = counter_q.separated(", ");
            for platform in query.platforms {
                separated.push_bind(platform);
                counter_separated.push_bind(platform);
            }
            q.push(") ");
            counter_q.push(") ");
        }

        if let Some(c) = query.compare {
            let sql = "AND SPLIT_PART(mv.version, '.', 1) = ";
            q.push(sql);
            counter_q.push(sql);
            let major = c.0.major.to_string();
            q.push_bind(major.clone());
            counter_q.push_bind(major.clone());
            let sql = " AND semver_compare(mv.version, ";
            q.push(sql);
            counter_q.push(sql);
            q.push_bind(c.0.to_string());
            counter_q.push_bind(c.0.to_string());
            let sql = match c.1 {
                ModVersionCompare::Exact => ") = 0",
                ModVersionCompare::Less => ") = -1",
                ModVersionCompare::LessEq => ") <= 0",
                ModVersionCompare::More => ") = 1",
                ModVersionCompare::MoreEq => ") >= 0",
            };
            q.push(sql);
            counter_q.push(sql);
        }

        let sql = "GROUP BY mv.id, mvs.status ORDER BY mv.id DESC LIMIT ";
        q.push(sql);
        q.push_bind(limit);
        let sql = " OFFSET ";
        q.push(sql);
        q.push_bind(offset);

        let records = q
            .build_query_as::<ModVersionGetOne>()
            .fetch_all(&mut *pool)
            .await
            .inspect_err(|e| log::error!("Failed to fetch index: {e}"))?;

        let count: i64 = counter_q
            .build_query_scalar()
            .fetch_one(&mut *pool)
            .await
            .inspect_err(|e| log::error!("Failed to fetch index count: {e}"))?;

        if records.is_empty() {
            return Ok(PaginatedData {
                data: vec![],
                count,
            });
        }

        let version_ids: Vec<i32> = records.iter().map(|x| x.id).collect();
        let deps = Dependency::get_for_mod_versions(&version_ids, None, None, None, pool).await?;
        let incompat =
            Incompatibility::get_for_mod_versions(&version_ids, None, None, None, pool).await?;

        let gd_versions = ModGDVersion::get_for_mod_versions(&version_ids, pool).await?;
        let ret: Vec<ModVersion> = records
            .into_iter()
            .map(|x| {
                let mut version = x.into_mod_version();
                version.gd = gd_versions.get(&version.id).cloned().unwrap_or_default();
                version.dependencies = Some(
                    deps.get(&version.id)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|j| j.to_response())
                        .collect(),
                );
                version.incompatibilities = Some(
                    incompat
                        .get(&version.id)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|j| j.to_response())
                        .collect(),
                );
                version
            })
            .collect();

        Ok(PaginatedData { data: ret, count })
    }

    pub async fn get_latest_for_mods(
        pool: &mut PgConnection,
        ids: &[String],
        gd: Option<GDVersionEnum>,
        platforms: Option<&[VerPlatform]>,
        geode: Option<&semver::Version>,
        requires_patching: Option<bool>,
    ) -> Result<HashMap<String, ModVersion>, DatabaseError> {
        if ids.is_empty() {
            return Ok(Default::default());
        }

        let gd_vec = gd.map(|x| vec![GDVersionEnum::All, x]);

        sqlx::query_as(
            "SELECT
                q.name, q.id, q.description, q.version,
                q.download_link, q.hash, q.geode,
                q.download_count, q.early_load, q.requires_patching, q.api, q.mod_id,
                'accepted'::mod_version_status as status,
                q.created_at, q.updated_at
            FROM (
                SELECT
                    mv.name, mv.id, mv.description, mv.version, mv.download_link, mv.hash,
                    format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as geode,
                    mv.download_count, mv.early_load, mv.requires_patching, mv.api, mv.mod_id, mv.created_at,
                    mv.updated_at,
                    ROW_NUMBER() OVER (PARTITION BY m.id ORDER BY mv.id DESC) rn
                FROM mods m
                INNER JOIN mod_versions mv ON m.id = mv.mod_id
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
                WHERE mvs.status = 'accepted'
                AND ($1 IS NULL OR mgv.gd = ANY($1))
                AND ($2 IS NULL OR mgv.platform = ANY($2))
                AND m.id = ANY($3)
                AND ($8 IS NULL OR mv.requires_patching = $8)
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
            ) q
            WHERE q.rn = 1"
        ).bind(gd_vec.as_ref())
        .bind(platforms)
        .bind(ids)
        .bind(geode.map(|x| i32::try_from(x.major).unwrap_or_default()))
        .bind(geode.map(|x| i32::try_from(x.minor).unwrap_or_default()))
        .bind(geode.map(|x| i32::try_from(x.patch).unwrap_or_default()))
        .bind(geode.map(|x| {
            if x.pre.is_empty() {
                None
            } else {
                Some(x.pre.to_string())
            }
        }))
        .bind(requires_patching)
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|x| log::error!("Failed to fetch latest versions for mods: {}", x))
        .map_err(|e| e.into())
        .map(|result: Vec<ModVersionGetOne>| {
            result.into_iter()
                .map(|i| (i.mod_id.clone(), i.into_mod_version()))
                .collect::<HashMap<_, _>>()
        })
    }

    pub async fn get_pending_for_mods(
        ids: &[String],
        pool: &mut PgConnection,
    ) -> Result<HashMap<String, Vec<ModVersion>>, DatabaseError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let records = sqlx::query_as!(
            ModVersionGetOne,
            r#"SELECT DISTINCT
                mv.name, mv.id, mv.description, mv.version, mv.download_link, mv.hash,
                format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as "geode!: _",
                mv.download_count, mv.early_load, mv.requires_patching, mv.api, mv.mod_id, mv.created_at, mv.updated_at,
                'pending'::mod_version_status as "status!: _", NULL as info
            FROM mod_versions mv
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE mvs.status = 'pending'
            AND mv.mod_id = ANY($1)
            ORDER BY mv.id DESC"#,
            ids
        ).fetch_all(&mut *pool)
        .await
        .inspect_err(|e| log::error!("Failed to fetch pending mod versions: {}", e))?;

        let mut ret: HashMap<String, Vec<ModVersion>> = HashMap::new();

        for x in records.into_iter() {
            let entry = ret.entry(x.mod_id.clone()).or_default();
            let version = x.into_mod_version();

            entry.push(version);
        }
        Ok(ret)
    }

    pub async fn get_latest_for_mod(
        id: &str,
        gd: Option<GDVersionEnum>,
        platforms: Vec<VerPlatform>,
        major: Option<u32>,
        pool: &mut PgConnection,
    ) -> Result<Option<ModVersion>, DatabaseError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT q.name, q.id, q.description, q.version, q.download_link,
                q.hash, q.geode, q.download_count,
                q.early_load, q.requires_patching, q.api, q.mod_id, q.status,
                q.created_at, q.updated_at
            FROM (
                SELECT mv.name, mv.id, mv.description, mv.version, mv.download_link,
                    mv.hash,
                    format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as geode,
                    mv.download_count, mvs.status,
                    mv.early_load, mv.requires_patching, mv.api, mv.mod_id, mv.created_at, mv.updated_at,
                    row_number() over (partition by m.id order by mv.id desc) rn
                FROM mods m
                INNER JOIN mod_versions mv ON m.id = mv.mod_id
                INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                WHERE mvs.status = 'accepted'"#,
        );
        if let Some(m) = major {
            let major_ver = format!("{}.%", m);
            query_builder.push(" AND mv.version LIKE ");
            query_builder.push_bind(major_ver);
        }
        if let Some(g) = gd {
            query_builder.push(" AND (mgv.gd = ");
            query_builder.push_bind(g);
            query_builder.push(" OR mgv.gd = ");
            query_builder.push_bind(GDVersionEnum::All);
            query_builder.push(")");
        }
        for (i, platform) in platforms.iter().enumerate() {
            if i == 0 {
                query_builder.push(" AND mgv.platform IN (");
            }
            query_builder.push_bind(*platform);
            if i == platforms.len() - 1 {
                query_builder.push(")");
            } else {
                query_builder.push(", ");
            }
        }
        query_builder.push(" AND mv.mod_id = ");
        query_builder.push_bind(id);
        query_builder.push(") q WHERE q.rn = 1");

        let version = query_builder
            .build_query_as::<ModVersionGetOne>()
            .fetch_optional(&mut *pool)
            .await
            .inspect_err(|e| log::error!("Failed to fetch latest mod_version for mod {id}: {e}"))?
            .map(|v| v.into_mod_version());

        let Some(mut version) = version else {
            return Ok(None);
        };

        let ids: Vec<i32> = vec![version.id];
        version.gd = ModGDVersion::get_for_mod_version(version.id, pool).await?;
        let geode = major.map(|major| Version::new(major.into(), 0, 0));
        version.dependencies = Some(
            Dependency::get_for_mod_versions(&ids, None, gd, geode.as_ref(), pool)
                .await?
                .get(&version.id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|x| x.to_response())
                .collect(),
        );
        version.incompatibilities = Some(
            Incompatibility::get_for_mod_version(version.id, pool)
                .await?
                .into_iter()
                .map(|x| x.to_response())
                .collect(),
        );
        version.developers = Some(developers::get_all_for_mod(&version.mod_id, pool).await?);
        version.tags = Some(Tag::get_tags_for_mod(&version.mod_id, pool).await?);

        Ok(Some(version))
    }

    pub async fn get_one(
        id: &str,
        version: &str,
        fetch_extras: bool,
        fetch_only_accepted: bool,
        pool: &mut PgConnection,
    ) -> Result<Option<ModVersion>, DatabaseError> {
        let result = sqlx::query_as!(
            ModVersionGetOne,
            r#"SELECT mv.id, mv.name, mv.description, mv.version,
                mv.download_link, mv.download_count,
                mv.hash,
                format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as "geode!: _",
                mv.early_load, mv.requires_patching, mv.api,
                mv.created_at, mv.updated_at,
                mv.mod_id, mvs.status as "status: _", mvs.info
            FROM mod_versions mv
            INNER JOIN mods m ON m.id = mv.mod_id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE mv.mod_id = $1 AND mv.version = $2
                AND (mvs.status = 'accepted' OR $3 = false)"#,
            id,
            version,
            fetch_only_accepted
        )
        .fetch_optional(&mut *pool)
        .await
        .inspect_err(|e| log::error!("ModVersion::get_one failed: {e}"))?
        .map(|x| x.into_mod_version());

        let Some(mut version) = result else {
            return Ok(None);
        };

        if fetch_extras {
            version.gd = ModGDVersion::get_for_mod_version(version.id, pool).await?;
            let ids = vec![version.id];
            version.dependencies = Some(
                Dependency::get_for_mod_versions(&ids, None, None, None, pool)
                    .await?
                    .get(&version.id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|x| x.to_response())
                    .collect(),
            );
            let incompat = Incompatibility::get_for_mod_version(version.id, pool).await?;
            version.incompatibilities =
                Some(incompat.into_iter().map(|x| x.to_response()).collect());
            version.developers = Some(developers::get_all_for_mod(&version.mod_id, pool).await?);
            version.tags = Some(Tag::get_tags_for_mod(&version.mod_id, pool).await?);
        }

        Ok(Some(version))
    }

    pub async fn get_accepted_count(
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<i64, DatabaseError> {
        sqlx::query_scalar!(
            "SELECT COUNT(*)
            FROM mod_versions mv
            INNER JOIN mod_version_statuses mvs ON mv.status_id = mvs.id
            WHERE mvs.status = 'accepted'
            AND mv.mod_id = $1",
            mod_id
        )
        .fetch_one(&mut *pool)
        .await
        .map(|x| x.unwrap_or_default())
        .map_err(|e| e.into())
    }
}
