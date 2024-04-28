use std::collections::HashMap;

use serde::Serialize;
use sqlx::{PgConnection, Postgres, QueryBuilder, Row};

use crate::types::{
    api::{create_download_link, ApiError},
    mod_json::{ModJson, ModJsonGDVersionType},
};

use super::{
    dependency::{Dependency, ResponseDependency},
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
    pub dependencies: Option<Vec<ResponseDependency>>,
    pub incompatibilities: Option<Vec<ResponseIncompatibility>>,
    pub developers: Option<Vec<Developer>>,
    pub tags: Option<Vec<String>>,
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
            gd: DetailedGDVersion {
                win: None,
                android: None,
                mac: None,
                ios: None,
                android32: None,
                android64: None,
            },
            developers: None,
            tags: None,
            dependencies: None,
            incompatibilities: None,
        }
    }
}

impl ModVersion {
    pub fn modify_download_link(&mut self, app_url: &str) {
        self.download_link = create_download_link(app_url, &self.mod_id, &self.version)
    }

    pub async fn get_latest_for_mods(
        pool: &mut PgConnection,
        ids: Vec<String>,
        gd: Option<GDVersionEnum>,
        platforms: Vec<VerPlatform>,
    ) -> Result<HashMap<String, ModVersion>, ApiError> {
        if ids.is_empty() {
            return Ok(Default::default());
        }

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT q.name, q.id, q.description, q.version, q.download_link, q.hash, q.geode, q.download_count,
                q.early_load, q.api, q.mod_id FROM (SELECT
                mv.name, mv.id, mv.description, mv.version, mv.download_link, mv.hash, mv.geode, mv.download_count,
                mv.early_load, mv.api, mv.mod_id, row_number() over (partition by m.id order by mv.id desc) rn FROM mods m 
                INNER JOIN mod_versions mv ON m.id = mv.mod_id
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
                WHERE mvs.status = 'accepted' 
            "#,
        );
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
        query_builder.push(" AND mv.mod_id IN (");
        let mut separated = query_builder.separated(",");
        for id in ids.iter() {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        query_builder.push(") q WHERE q.rn = 1");
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

        let mut ret: HashMap<String, ModVersion> = HashMap::new();

        for x in records.into_iter() {
            let mod_id = x.mod_id.clone();
            let version = x.into_mod_version();
            ret.insert(mod_id, version);
        }
        Ok(ret)
    }

    // WIP

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
            mv.early_load, mv.api, mv.mod_id FROM mod_versions mv 
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
            r#"SELECT q.name, q.id, q.description, q.version, q.download_link, q.hash, q.geode, q.download_count,
            q.early_load, q.api, q.mod_id FROM (SELECT
            mv.name, mv.id, mv.description, mv.version, mv.download_link, mv.hash, mv.geode, mv.download_count,
            mv.early_load, mv.api, mv.mod_id, row_number() over (partition by m.id order by mv.id desc) rn FROM mods m 
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE mvs.status = 'accepted'"#,
        );
        if let Some(m) = major {
            let major_ver = format!("{}.%", m);
            query_builder.push(" AND mgv.version LIKE ");
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
        let records = match query_builder
            .build_query_as::<ModVersionGetOne>()
            .fetch_optional(&mut *pool)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::info!("{:?}", e);
                return Err(ApiError::DbError);
            }
        };
        let mut version = match records {
            None => return Err(ApiError::NotFound("".to_string())),
            Some(x) => x.into_mod_version(),
        };

        version.gd = ModGDVersion::get_for_mod_version(version.id, pool).await?;
        version.dependencies = Some(
            Dependency::get_for_mod_version(version.id, pool)
                .await?
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

    pub async fn get_download_url(
        id: &str,
        version: &str,
        pool: &mut PgConnection,
    ) -> Result<String, ApiError> {
        let result = sqlx::query!(
            "SELECT mv.download_link FROM mod_versions mv
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE mv.mod_id = $1 AND mv.version = $2 AND mvs.status = 'accepted'",
            id,
            version
        )
        .fetch_optional(&mut *pool)
        .await;
        if result.is_err() {
            return Err(ApiError::DbError);
        }
        match result.unwrap() {
            None => Err(ApiError::NotFound(format!(
                "Mod {}, version {} doesn't exist",
                id, version
            ))),
            Some(r) => Ok(r.download_link),
        }
    }

    pub async fn create_from_json(
        json: &ModJson,
        dev_verified: bool,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if let Err(e) = sqlx::query!("SET CONSTRAINTS mod_versions_status_id_fkey DEFERRED")
            .execute(&mut *pool)
            .await
        {
            log::error!("{}", e);
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
        match &json.gd {
            ModJsonGDVersionType::VersionStr(ver) => {
                ModGDVersion::create_for_all_platforms(json, *ver, id, pool).await?
            }
            ModJsonGDVersionType::VersionObj(vec) => {
                ModGDVersion::create_from_json(vec.to_create_payload(json), id, pool).await?;
            }
        }
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

        let status = if dev_verified {
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

        if let Err(e) = sqlx::query!("SET CONSTRAINTS mod_versions_status_id_fkey IMMEDIATE")
            .execute(&mut *pool)
            .await
        {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        };

        Ok(())
    }

    pub async fn get_one(
        id: &str,
        version: &str,
        fetch_extras: bool,
        pool: &mut PgConnection,
    ) -> Result<ModVersion, ApiError> {
        let result = sqlx::query_as!(
            ModVersionGetOne,
            "SELECT
            mv.id, mv.name, mv.description, mv.version, mv.download_link, mv.download_count,
            mv.hash, mv.geode, mv.early_load, mv.api, mv.mod_id FROM mod_versions mv
            INNER JOIN mods m ON m.id = mv.mod_id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id 
            WHERE mv.mod_id = $1 AND mv.version = $2 AND mvs.status = 'accepted'",
            id,
            version
        )
        .fetch_optional(&mut *pool)
        .await;

        let result = match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };
        if result.is_none() {
            return Err(ApiError::NotFound("Not found".to_string()));
        }

        let mut version = result.unwrap().into_mod_version();
        if fetch_extras {
            version.gd = ModGDVersion::get_for_mod_version(version.id, pool).await?;
            let deps = Dependency::get_for_mod_version(version.id, pool).await?;
            version.dependencies = Some(deps.into_iter().map(|x| x.to_response()).collect());
            let incompat = Incompatibility::get_for_mod_version(version.id, pool).await?;
            version.incompatibilities =
                Some(incompat.into_iter().map(|x| x.to_response()).collect());
            version.developers = Some(Developer::fetch_for_mod(&version.mod_id, pool).await?);
            version.tags = Some(Tag::get_tags_for_mod(&version.mod_id, pool).await?);
        }

        Ok(version)
    }

    pub async fn calculate_cached_downloads(
        mod_version_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if let Err(e) = sqlx::query!(
            "UPDATE mod_versions mv 
            SET download_count = mv.download_count + (
                SELECT COUNT(DISTINCT md.ip) FROM mod_downloads md
                WHERE md.mod_version_id = mv.id AND md.time_downloaded > mv.last_download_cache_refresh 
            ), last_download_cache_refresh = now()
            FROM mod_version_statuses mvs
            WHERE mv.id = $1 AND mvs.mod_version_id = mv.id AND mvs.status = 'accepted'",
            mod_version_id
        )
        .execute(&mut *pool)
        .await {
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

        Ok(())
    }
}
