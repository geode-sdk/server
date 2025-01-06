use std::collections::HashMap;

use chrono::SecondsFormat;
use semver::Version;
use serde::Serialize;
use sqlx::{
    types::chrono::{DateTime, Utc},
    PgConnection, Postgres, QueryBuilder, Row,
};

use crate::types::{
    api::{create_download_link, ApiError, PaginatedData},
    mod_json::ModJson,
    models::{download, mod_entity::Mod},
};

use super::{
    dependency::{Dependency, ModVersionCompare, ResponseDependency},
    developer::Developer,
    incompatibility::{Incompatibility, ResponseIncompatibility},
    mod_gd_version::{DetailedGDVersion, GDVersionEnum, ModGDVersion, VerPlatform},
    mod_version_status::{ModVersionStatus, ModVersionStatusEnum},
    tag::Tag,
};

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
    pub api: bool,
    pub mod_id: String,
    pub gd: DetailedGDVersion,
    pub status: ModVersionStatusEnum,
    pub dependencies: Option<Vec<ResponseDependency>>,
    pub incompatibilities: Option<Vec<ResponseIncompatibility>>,
    pub developers: Option<Vec<Developer>>,
    pub tags: Option<Vec<String>>,

    pub created_at: Option<String>,
    pub updated_at: Option<String>,

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
            created_at: self
                .created_at
                .map(|x| x.to_rfc3339_opts(SecondsFormat::Secs, true)),
            updated_at: self
                .updated_at
                .map(|x| x.to_rfc3339_opts(SecondsFormat::Secs, true)),
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
    ) -> Result<PaginatedData<ModVersion>, ApiError> {
        let limit = query.per_page;
        let offset = (query.page - 1) * query.per_page;

        let mut q: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT mv.id, mv.name, mv.description, mv.version,
            mv.download_link, mv.download_count, mv.hash, mv.geode,
            mv.early_load, mv.api, mv.mod_id, mvs.status, mv.created_at, mv.updated_at
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

        let records = match q
            .build_query_as::<ModVersionGetOne>()
            .fetch_all(&mut *pool)
            .await
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        let count: i64 = match counter_q.build_query_scalar().fetch_one(&mut *pool).await {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(c) => c,
        };

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
        ids: Vec<String>,
        gd: Option<GDVersionEnum>,
        platforms: Vec<VerPlatform>,
        geode: Option<&String>,
    ) -> Result<HashMap<String, ModVersion>, ApiError> {
        if ids.is_empty() {
            return Ok(Default::default());
        }

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT q.name, q.id, q.description, q.version, q.download_link, q.hash, q.geode, q.download_count,
                q.early_load, q.api, q.mod_id, q.status, q.created_at, q.updated_at FROM (
                    SELECT
                    mv.name, mv.id, mv.description, mv.version, mv.download_link, mv.hash, mv.geode, mv.download_count, mvs.status,
                    mv.early_load, mv.api, mv.mod_id, mv.created_at, mv.updated_at, row_number() over (partition by m.id order by mv.id desc) rn FROM mods m 
                    INNER JOIN mod_versions mv ON m.id = mv.mod_id
                    INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                    INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
                    WHERE mvs.status = 'accepted' 
            "#,
        );
        if let Some(g) = gd {
            builder.push(" AND (mgv.gd = ");
            builder.push_bind(g);
            builder.push(" OR mgv.gd = ");
            builder.push_bind(GDVersionEnum::All);
            builder.push(")");
        }

        if let Some(geode) = geode {
            let geode = geode.trim_start_matches('v').to_string();
            if let Ok(parsed) = Version::parse(&geode) {
                // If alpha, match exactly that version
                if parsed.pre.contains("alpha") {
                    let sql = " AND mv.geode = ";
                    builder.push(sql);
                    builder.push_bind(parsed.to_string());
                } else {
                    let sql = " AND (SPLIT_PART(mv.geode, '.', 1) = ";
                    builder.push(sql);
                    builder.push_bind(parsed.major.to_string());

                    let sql = " AND SPLIT_PART(mv.geode, '-', 2) NOT LIKE 'alpha%' AND SPLIT_PART(mv.geode, '.', 2) <= ";
                    builder.push(sql);
                    builder.push_bind(parsed.minor.to_string());

                    // Match only higher betas (or no beta)
                    if parsed.pre.contains("beta") {
                        let sql = " AND (SPLIT_PART(mv.geode, '-', 2) = ''
                            OR SPLIT_PART(mv.geode, '-', 2) <=";
                        builder.push(sql);
                        builder.push_bind(parsed.pre.to_string());
                        builder.push(")");
                    }

                    builder.push(")");
                }
            }
        }

        for (i, platform) in platforms.iter().enumerate() {
            if i == 0 {
                builder.push(" AND mgv.platform IN (");
            }
            builder.push_bind(*platform);
            if i == platforms.len() - 1 {
                builder.push(")");
            } else {
                builder.push(", ");
            }
        }
        builder.push(" AND mv.mod_id IN (");
        let mut separated = builder.separated(",");
        for id in ids.iter() {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        builder.push(") q WHERE q.rn = 1");
        let records = builder
            .build_query_as::<ModVersionGetOne>()
            .fetch_all(&mut *pool)
            .await;
        let records = match records {
            Err(e) => {
                log::info!("{:?}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        let mut ret: HashMap<String, ModVersion> = HashMap::new();

        for x in records.into_iter() {
            let mod_id = x.mod_id.clone();
            let version = x.into_mod_version();
            ret.insert(mod_id, version);
        }
        Ok(ret)
    }

    pub async fn get_pending_for_mods(
        ids: &Vec<String>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<String, Vec<ModVersion>>, ApiError> {
        if ids.is_empty() {
            return Ok(Default::default());
        }

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT DISTINCT
            mv.name, mv.id, mv.description, mv.version, mv.download_link, mv.hash, mv.geode, mv.download_count,
            mv.early_load, mv.api, mv.mod_id, mv.created_at, mv.updated_at, mvs.status FROM mod_versions mv 
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE mvs.status = 'pending' AND mv.mod_id IN ("#,
        );
        let mut separated = query_builder.separated(",");

        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        let records = query_builder
            .build_query_as::<ModVersionGetOne>()
            .fetch_all(&mut *pool)
            .await;
        let records = match records {
            Err(e) => {
                log::info!("{:?}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        let mut ret: HashMap<String, Vec<ModVersion>> = HashMap::new();

        for x in records.into_iter() {
            let mod_id = x.mod_id.clone();
            let version = x.into_mod_version();

            ret.entry(mod_id).or_default().push(version);
        }
        Ok(ret)
    }

    pub async fn get_latest_for_mod(
        id: &str,
        gd: Option<GDVersionEnum>,
        platforms: Vec<VerPlatform>,
        major: Option<u32>,
        pool: &mut PgConnection,
    ) -> Result<ModVersion, ApiError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT q.name, q.id, q.description, q.version, q.download_link, 
                q.hash, q.geode, q.download_count,
                q.early_load, q.api, q.mod_id, q.status,
                q.created_at, q.updated_at
            FROM (
                SELECT mv.name, mv.id, mv.description, mv.version, mv.download_link, 
                    mv.hash, mv.geode, mv.download_count, mvs.status,
                    mv.early_load, mv.api, mv.mod_id, mv.created_at, mv.updated_at,
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
        let mut version = match query_builder
            .build_query_as::<ModVersionGetOne>()
            .fetch_optional(&mut *pool)
            .await
        {
            Ok(Some(r)) => r.into_mod_version(),
            Ok(None) => {
                return Err(ApiError::NotFound("".to_string()));
            }
            Err(e) => {
                log::error!("{:?}", e);
                return Err(ApiError::DbError);
            }
        };

        let ids: Vec<i32> = vec![version.id];
        version.gd = ModGDVersion::get_for_mod_version(version.id, pool).await?;
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
        version.incompatibilities = Some(
            Incompatibility::get_for_mod_version(version.id, pool)
                .await?
                .into_iter()
                .map(|x| x.to_response())
                .collect(),
        );
        version.developers = Some(Developer::fetch_for_mod(&version.mod_id, pool).await?);
        version.tags = Some(Tag::get_tags_for_mod(&version.mod_id, pool).await?);

        Ok(version)
    }

    pub async fn create_from_json(
        json: &ModJson,
        make_accepted: bool,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if let Err(e) = sqlx::query!("SET CONSTRAINTS mod_versions_status_id_fkey DEFERRED")
            .execute(&mut *pool)
            .await
        {
            log::error!(
                "Error while updating constraints for mod_version_statuses: {}",
                e
            );
            return Err(ApiError::DbError);
        };

        // If someone finds a way to use macros with optional parameters you can impl it here
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO mod_versions (");
        if json.description.is_some() {
            builder.push("description, ");
        }
        builder
            .push("name, version, download_link, hash, geode, early_load, api, mod_id, status_id) VALUES (");
        let mut separated = builder.separated(", ");
        if json.description.is_some() {
            separated.push_bind(&json.description);
        }
        separated.push_bind(&json.name);
        separated.push_bind(&json.version);
        separated.push_bind(&json.download_url);
        separated.push_bind(&json.hash);
        separated.push_bind(&json.geode);
        separated.push_bind(json.early_load);
        separated.push_bind(json.api.is_some());
        separated.push_bind(&json.id);
        // set status_id = 0, will be checked by foreign key at the end of the transaction
        separated.push_bind(0);
        separated.push_unseparated(") RETURNING id");
        let result = builder.build().fetch_one(&mut *pool).await;
        let result = match result {
            Err(e) => {
                log::error!("{:?}", e);
                return Err(ApiError::DbError);
            }
            Ok(row) => row,
        };
        let id = result.get::<i32, &str>("id");
        let json_tags = json.tags.clone().unwrap_or_default();
        let tags = Tag::get_tag_ids(json_tags, pool).await?;
        Tag::update_mod_tags(&json.id, tags.into_iter().map(|x| x.id).collect(), pool).await?;
        ModGDVersion::create_from_json(json.gd.to_create_payload(json), id, pool).await?;
        if json.dependencies.as_ref().is_some_and(|x| !x.is_empty()) {
            let dependencies = json.prepare_dependencies_for_create()?;
            if !dependencies.is_empty() {
                Dependency::create_for_mod_version(id, dependencies, pool).await?;
            }
        }
        if json
            .incompatibilities
            .as_ref()
            .is_some_and(|x| !x.is_empty())
        {
            let incompat = json.prepare_incompatibilities_for_create()?;
            if !incompat.is_empty() {
                Incompatibility::create_for_mod_version(id, incompat, pool).await?;
            }
        }

        let status = if make_accepted {
            ModVersionStatusEnum::Accepted
        } else {
            ModVersionStatusEnum::Pending
        };

        let status_id =
            ModVersionStatus::create_for_mod_version(id, status, None, None, pool).await?;
        if let Err(e) = sqlx::query!(
            "update mod_versions set status_id = $1 where id = $2",
            status_id,
            id
        )
        .execute(&mut *pool)
        .await
        {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }

        // Revert deferred constraints
        if let Err(e) = sqlx::query!("SET CONSTRAINTS mod_versions_status_id_fkey IMMEDIATE")
            .execute(&mut *pool)
            .await
        {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        };

        Ok(())
    }

    pub async fn update_pending_version(
        version_id: i32,
        json: &ModJson,
        make_accepted: bool,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let result = sqlx::query!(
            "UPDATE mod_versions mv
                SET name = $1,
                version = $2,
                download_link = $3,
                hash = $4,
                geode = $5,
                early_load = $6,
                api = $7,
                description = $8,
                updated_at = NOW()
            FROM mod_version_statuses mvs
            WHERE mv.status_id = mvs.id
            AND mvs.status = 'pending'
            AND mv.id = $9",
            &json.name,
            &json.version,
            &json.download_url,
            &json.hash,
            &json.geode,
            &json.early_load,
            &json.api.is_some(),
            json.description.clone().unwrap_or_default(),
            version_id
        )
        .execute(&mut *pool)
        .await
        .map_err(|err| {
            log::error!(
                "Failed to update pending version {}-{}: {}",
                json.id,
                json.version,
                err
            );
            ApiError::DbError
        })?;

        let json_tags = json.tags.clone().unwrap_or_default();
        let tags = Tag::get_tag_ids(json_tags, pool).await?;
        Tag::update_mod_tags(&json.id, tags.into_iter().map(|x| x.id).collect(), pool).await?;
        ModGDVersion::clear_for_mod_version(version_id, pool).await.map_err(|err| {
            log::error!("{}", err);
            ApiError::DbError
        })?;
        ModGDVersion::create_from_json(json.gd.to_create_payload(json), version_id, pool).await?;
        Dependency::clear_for_mod_version(version_id, pool).await?;
        Incompatibility::clear_for_mod_version(version_id, pool).await?;

        if json.dependencies.as_ref().is_some_and(|x| !x.is_empty()) {
            let dependencies = json.prepare_dependencies_for_create()?;
            if !dependencies.is_empty() {
                Dependency::create_for_mod_version(version_id, dependencies, pool).await?;
            }
        }
        if json
            .incompatibilities
            .as_ref()
            .is_some_and(|x| !x.is_empty())
        {
            let incompat = json.prepare_incompatibilities_for_create()?;
            if !incompat.is_empty() {
                Incompatibility::create_for_mod_version(version_id, incompat, pool).await?;
            }
        }

        let status = if make_accepted {
            ModVersionStatusEnum::Accepted
        } else {
            ModVersionStatusEnum::Pending
        };

        ModVersionStatus::update_for_mod_version(version_id, status, None, None, pool).await?;
        Ok(())
    }

    pub async fn get_one(
        id: &str,
        version: &str,
        fetch_extras: bool,
        fetch_only_accepted: bool,
        pool: &mut PgConnection,
    ) -> Result<ModVersion, ApiError> {
        let result = match sqlx::query_as!(
            ModVersionGetOne,
            r#"SELECT mv.id, mv.name, mv.description, mv.version, 
                mv.download_link, mv.download_count,
                mv.hash, mv.geode, mv.early_load, mv.api,
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
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(None) => return Err(ApiError::NotFound("Not found".to_string())),
            Ok(Some(r)) => r,
        };

        let mut version = result.into_mod_version();
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
            version.developers = Some(Developer::fetch_for_mod(&version.mod_id, pool).await?);
            version.tags = Some(Tag::get_tags_for_mod(&version.mod_id, pool).await?);
        }

        Ok(version)
    }

    pub async fn increment_downloads(
        mod_version_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        // we aren't recalculating, so don't reset the timer
        if let Err(e) = sqlx::query!(
            "UPDATE mod_versions mv
            SET download_count = mv.download_count + 1
            FROM mod_version_statuses mvs
            WHERE mv.id = $1 AND mvs.mod_version_id = mv.id AND mvs.status = 'accepted'",
            mod_version_id
        )
        .execute(&mut *pool)
        .await
        {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(())
    }

    pub async fn calculate_cached_downloads(
        mod_version_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if let Err(e) = sqlx::query!(
            "UPDATE mod_versions mv 
            SET download_count = (
                SELECT COUNT(DISTINCT md.ip) FROM mod_downloads md
                WHERE md.mod_version_id = mv.id
            ), last_download_cache_refresh = now()
            FROM mod_version_statuses mvs
            WHERE mv.id = $1 AND mvs.mod_version_id = mv.id AND mvs.status = 'accepted'",
            mod_version_id
        )
        .execute(&mut *pool)
        .await
        {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(())
    }

    pub async fn update_version(
        id: i32,
        new_status: ModVersionStatusEnum,
        info: Option<String>,
        admin_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        struct CurrentStatusRes {
            status: ModVersionStatusEnum,
        }
        let current_status = match sqlx::query_as!(
            CurrentStatusRes,
            r#"select status as "status: _" from mod_version_statuses
            where mod_version_id = $1"#,
            id
        )
        .fetch_one(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(s) => s,
        };

        if current_status.status == new_status {
            return Ok(());
        }

        if current_status.status == ModVersionStatusEnum::Accepted
            && new_status == ModVersionStatusEnum::Pending
        {
            return Err(ApiError::BadRequest(
                "Cannot turn an accepted mod back into pending".to_string(),
            ));
        }

        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("UPDATE mod_version_statuses SET ");

        query_builder.push("status = ");
        query_builder.push_bind(new_status);
        query_builder.push(", admin_id = ");
        query_builder.push_bind(admin_id);
        if let Some(i) = info {
            query_builder.push(", info = ");
            query_builder.push_bind(i);
        }

        query_builder.push(" WHERE mod_version_id = ");
        query_builder.push_bind(id);

        if let Err(e) = query_builder.build().execute(&mut *pool).await {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }

        if download::downloaded_version(id, pool).await? {
            // performing this operation will change the mod's download count, so recalculate it

            // should probably spawn this, but we do a download in the transaction which is probably
            // a little worse. idk

            let info = match sqlx::query!("SELECT mod_id FROM mod_versions WHERE id = $1", id)
                .fetch_one(&mut *pool)
                .await
            {
                Err(e) => {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                }
                Ok(r) => r,
            };

            ModVersion::calculate_cached_downloads(id, pool).await?;
            Mod::calculate_cached_downloads(&info.mod_id, pool).await?;
        }

        if current_status.status == ModVersionStatusEnum::Pending
            && new_status == ModVersionStatusEnum::Accepted
        {
            // Time to download that image
            let info = match sqlx::query!(
                "SELECT download_link, hash, mod_id FROM mod_versions WHERE id = $1",
                id
            )
            .fetch_one(&mut *pool)
            .await
            {
                Err(e) => {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                }
                Ok(r) => r,
            };
            Mod::update_mod_image(&info.mod_id, &info.hash, &info.download_link, pool).await?;
        }

        if new_status == ModVersionStatusEnum::Accepted {
            match sqlx::query!(
                "UPDATE mods m
            SET updated_at = $1
            WHERE m.id = (select mv.mod_id from mod_versions mv where mv.id = $2)",
                Utc::now(),
                id
            )
            .execute(&mut *pool)
            .await
            {
                Err(e) => {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                }
                Ok(r) => {
                    if r.rows_affected() == 0 {
                        log::error!("No mods affected by updated_at update.");
                    }
                }
            };
        }

        match sqlx::query!(
            "UPDATE mod_versions SET updated_at=$1 WHERE id=$2",
            Utc::now(),
            id
        )
        .execute(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        Ok(())
    }

    pub async fn get_accepted_count(
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<i64, ApiError> {
        let count = match sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mod_versions mv INNER JOIN mod_version_statuses mvs ON mv.status_id = mvs.id WHERE mvs.status = 'accepted' AND mv.mod_id = $1",
            mod_id
        )
        .fetch_one(&mut *pool)
        .await
        {
            Ok(Some(count)) => count,
            Ok(None) => 0,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        Ok(count)
    }
}
