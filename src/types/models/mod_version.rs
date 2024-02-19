use std::{
    collections::{hash_map::Entry, HashMap},
    vec,
};

use serde::Serialize;
use sqlx::{PgConnection, Postgres, QueryBuilder, Row};

use crate::types::{
    api::ApiError,
    mod_json::{ModJson, ModJsonGDVersionType},
};

use super::{
    dependency::{Dependency, ResponseDependency},
    incompatibility::{Incompatibility, ResponseIncompatibility},
    mod_gd_version::{DetailedGDVersion, GDVersionEnum, ModGDVersion, VerPlatform},
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
    pub early_load: bool,
    pub api: bool,
    pub mod_id: String,
    pub gd: DetailedGDVersion,
    pub dependencies: Option<Vec<ResponseDependency>>,
    pub incompatibilities: Option<Vec<ResponseIncompatibility>>,
}

#[derive(sqlx::FromRow)]
struct ModVersionGetOne {
    id: i32,
    name: String,
    description: Option<String>,
    version: String,
    download_link: String,
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
            dependencies: None,
            incompatibilities: None,
        }
    }
}

impl ModVersion {
    pub fn modify_download_link(&mut self, app_url: &str) {
        self.download_link = format!(
            "{}/v1/mods/{}/versions/{}/download",
            app_url, self.mod_id, self.version
        );
    }
    pub async fn get_latest_for_mods(
        pool: &mut PgConnection,
        ids: &[&str],
        gd: Option<GDVersionEnum>,
        platforms: Vec<VerPlatform>,
    ) -> Result<HashMap<String, Vec<ModVersion>>, ApiError> {
        if ids.is_empty() {
            return Ok(Default::default());
        }

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT DISTINCT
            mv.name, mv.id, mv.description, mv.version, mv.download_link, mv.hash, mv.geode,
            mv.early_load, mv.api, mv.mod_id FROM mod_versions mv 
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
            INNER JOIN mods m ON m.id = mv.mod_id
            WHERE mv.version = m.latest_version AND mv.validated = true"#,
        );
        if let Some(g) = gd {
            query_builder.push(" AND mgv.gd = ");
            query_builder.push_bind(g);
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
            match ret.entry(mod_id) {
                Entry::Vacant(e) => {
                    let vector: Vec<ModVersion> = vec![version];
                    e.insert(vector);
                }
                Entry::Occupied(mut e) => e.get_mut().push(version),
            }
        }
        Ok(ret)
    }

    pub async fn get_download_url(
        id: &str,
        version: &str,
        pool: &mut PgConnection,
    ) -> Result<String, ApiError> {
        let result = sqlx::query!("SELECT download_link FROM mod_versions WHERE mod_id = $1 AND version = $2 AND validated = true", id, version)
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
        // If someone finds a way to use macros with optional parameters you can impl it here
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO mod_versions (");
        if json.description.is_some() {
            builder.push("description, ");
        }
        builder.push("name, version, download_link, validated, hash, geode, early_load, api, mod_id) VALUES (");
        let mut separated = builder.separated(", ");
        if json.description.is_some() {
            separated.push_bind(&json.description);
        }
        separated.push_bind(&json.name);
        separated.push_bind(&json.version);
        separated.push_bind(&json.download_url);
        separated.push_bind(dev_verified);
        separated.push_bind(&json.hash);
        separated.push_bind(&json.geode);
        separated.push_bind(json.early_load);
        separated.push_bind(json.api);
        separated.push_bind(&json.id);
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
        Ok(())
    }

    pub async fn get_one(
        id: &str,
        version: &str,
        pool: &mut PgConnection,
    ) -> Result<ModVersion, ApiError> {
        let result = sqlx::query_as!(
            ModVersionGetOne,
            "SELECT
            mv.id, mv.name, mv.description, mv.version, mv.download_link,
            mv.hash, mv.geode, mv.early_load, mv.api, mv.mod_id FROM mod_versions mv
            INNER JOIN mods m ON m.id = mv.mod_id
            WHERE mv.mod_id = $1 AND mv.version = $2 AND mv.validated = true",
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
        version.gd = ModGDVersion::get_for_mod_version(version.id, pool).await?;
        let deps = Dependency::get_for_mod_version(&version, pool).await?;
        version.dependencies = Some(
            deps.into_iter()
                .map(|x| ResponseDependency {
                    mod_id: x.dependency_id.clone(),
                    version: format!("{}{}", x.compare, x.version.trim_start_matches('v')),
                    importance: x.importance,
                })
                .collect(),
        );
        let incompat = Incompatibility::get_for_mod_version(version.id, pool).await?;
        version.incompatibilities = Some(
            incompat
                .into_iter()
                .map(|x| ResponseIncompatibility {
                    mod_id: x.incompatibility_id.clone(),
                    version: format!("{}{}", x.compare, x.version.trim_start_matches('v')),
                    importance: x.importance,
                })
                .collect(),
        );

        Ok(version)
    }

    pub async fn calculate_cached_downloads(
        mod_version_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if let Err(e) = sqlx::query!(
            "UPDATE mod_versions mv SET download_count = mv.download_count + (
                SELECT COUNT(DISTINCT md.ip) FROM mod_downloads md
                WHERE md.mod_version_id = mv.id AND md.time_downloaded > mv.last_download_cache_refresh 
            ), last_download_cache_refresh = now()
            WHERE mv.id = $1 AND mv.validated = true",
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
        id: &str,
        version: &str,
        validated: Option<bool>,
        unlisted: Option<bool>,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if validated.is_none() && unlisted.is_none() {
            return Ok(());
        }

        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("UPDATE mod_versions SET ");

        if let Some(v) = validated {
            query_builder.push("validated = ");
            query_builder.push_bind(v);
        }

        query_builder.push("WHERE mod_id = ");
        query_builder.push_bind(id);
        query_builder.push(" AND version = ");
        let version = version.trim_start_matches('v');
        query_builder.push_bind(version);

        let result = query_builder.build().execute(&mut *pool).await;

        match result {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(r) => {
                if r.rows_affected() == 0 {
                    return Err(ApiError::NotFound(format!("{} {} not found", id, version)));
                }
                Ok(())
            }
        }
    }
}
