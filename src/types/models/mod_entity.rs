use actix_web::web::Bytes;
use serde::Serialize;
use sqlx::{PgConnection, QueryBuilder, Postgres};
use std::{io::Cursor, str::FromStr};
use crate::{types::{models::mod_version::ModVersion, api::{PaginatedData, ApiError}, mod_json::ModJson}, endpoints::mods::IndexQueryParams};

use super::mod_gd_version::{DetailedGDVersion, ModGDVersion, VerPlatform};

#[derive(Serialize, Debug, sqlx::FromRow)]
pub struct Mod {
    pub id: String,
    pub repository: Option<String>,
    pub latest_version: String,
    pub validated: bool,
    pub versions: Vec<ModVersion>
}

#[derive(Debug, sqlx::FromRow)]
struct ModRecord {
    id: String,
    #[sqlx(default)]
    repository: Option<String>,
    latest_version: String,
    validated: bool,
}

#[derive(sqlx::FromRow)]
struct ModRecordGetOne {
    id: String,
    repository: Option<String>,
    latest_version: String,
    validated: bool,
    version_id: i32,
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

impl Mod {
    pub async fn get_index(pool: &mut PgConnection, query: IndexQueryParams) -> Result<PaginatedData<Mod>, ApiError> {
        let page = query.page.unwrap_or(1);
        let per_page = query.per_page.unwrap_or(10);
        let limit = per_page;
        let offset = (page - 1) * per_page;
        let query_string = format!("%{}%", query.query.unwrap_or("".to_string()));
        let mut platforms: Vec<VerPlatform> = vec![];
        if query.platforms.is_some() {
            for i in query.platforms.unwrap().split(",") {
                let trimmed = i.trim();
                let platform = VerPlatform::from_str(trimmed).or(Err(ApiError::BadRequest(format!("Invalid platform {}", trimmed))))?;
                if platform == VerPlatform::Android {
                    platforms.push(VerPlatform::Android32);
                    platforms.push(VerPlatform::Android64);
                } else {
                    platforms.push(platform)
                }
            }
        }
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT DISTINCT m.id, m.repository, m.latest_version, m.validated FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
            WHERE m.validated = true AND mv.name LIKE "
        );
        let mut counter_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT COUNT(*) FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
            WHERE m.validated = true AND mv.name LIKE "
        );
        counter_builder.push_bind(&query_string);
        builder.push_bind(&query_string);
        match query.gd {
            Some(g) => {
                builder.push(" AND mgv.gd = ");
                builder.push_bind(g);
                counter_builder.push(" AND mgv.gd = ");
                counter_builder.push_bind(g);
            },
            None => ()
        };
        for (i, platform) in platforms.iter().enumerate() {
            if i == 0 {
                builder.push(" AND mgv.platform IN (");
                counter_builder.push(" AND mgv.platform IN (");
            }
            builder.push_bind(platform.clone());
            counter_builder.push_bind(platform.clone());
            if i == platforms.len() - 1 {
                builder.push(")");
                counter_builder.push(")");
            } else {
                builder.push(", ");
                counter_builder.push(", ");
            }
        }
        builder.push(" LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let result = builder.build_query_as::<ModRecord>()
            .fetch_all(&mut *pool)
            .await;
        let records = match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            },
            Ok(r) => r
        };

        let result = counter_builder.build_query_scalar()
            .fetch_one(&mut *pool)
            .await;
        let count = match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            },
            Ok(c) => c
        };

        if records.is_empty() {
            return Ok(PaginatedData { data: vec![], count: 0 });
        }

        let ids: Vec<_> = records.iter().map(|x| x.id.as_str()).collect();
        let versions = ModVersion::get_latest_for_mods(pool, &ids, query.gd, platforms).await?;
        let mut mod_version_ids: Vec<i32> = vec![];
        for i in &versions {
            let mut version_ids: Vec<_> = i.1.iter().map(|x| { x.id }).collect();
            mod_version_ids.append(&mut version_ids);
        }

        let gd_versions = ModGDVersion::get_for_mod_versions(mod_version_ids, pool).await?;

        let ret = records.into_iter().map(|x| {
            let mut version_vec = versions.get(&x.id).cloned().unwrap_or_default();
            for i in &mut version_vec {
                let gd_ver = gd_versions.get(&i.id).cloned().unwrap_or_default();
                i.gd = gd_ver;
            }
            Mod {
                id: x.id.clone(),
                repository: x.repository.clone(),
                latest_version: x.latest_version.clone(),
                validated: x.validated,
                versions: version_vec
            }
        }).collect();
        Ok(PaginatedData{ data: ret, count })
    }

    pub async fn get_one(id: &str, pool: &mut PgConnection) -> Result<Option<Mod>, ApiError> {
        let records: Vec<ModRecordGetOne> = sqlx::query_as!(ModRecordGetOne, 
            "SELECT
                m.id, m.repository, m.latest_version, m.validated,
                mv.id as version_id, mv.name, mv.description, mv.version, mv.download_link,
                mv.hash, mv.geode, mv.early_load, mv.api, mv.mod_id
            FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            WHERE m.id = $1",
            id
        ).fetch_all(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
        if records.len() == 0 {
            return Ok(None);
        }
        let mut versions: Vec<ModVersion> = records.iter().map(|x| {
            ModVersion {
                id: x.version_id,
                name: x.name.clone(),
                description: x.description.clone(),
                version: x.version.clone(),
                download_link: x.download_link.clone(),
                hash: x.hash.clone(),
                geode: x.geode.clone(),
                early_load: x.early_load,
                api: x.api,
                mod_id: x.mod_id.clone(),
                gd: DetailedGDVersion { win: None, android: None, mac: None, ios: None, android32: None, android64: None },
                changelog: None,
                about: None,
                dependencies: None,
                incompatibilities: None
            }
        }).collect();
        let ids = versions.iter().map(|x| {x.id}).collect();
        let gd = ModGDVersion::get_for_mod_versions(ids, pool).await?;
        for (id, gd_versions) in &gd {
            for i in &mut versions {
                if &i.id == id {
                    i.gd = gd_versions.clone();
                } 
            }
        }

        let mod_entity = Mod {
            id: records[0].id.clone(),
            repository: records[0].repository.clone(),
            latest_version: records[0].latest_version.clone(),
            validated: records[0].validated,
            versions
        };
        Ok(Some(mod_entity))
    }

    pub async fn from_json(json: &ModJson, pool: &mut PgConnection) -> Result<(), ApiError> {
        if semver::Version::parse(json.version.trim_start_matches("v")).is_err() {
            return Err(ApiError::BadRequest(format!("Invalid mod version semver {}", json.version)));
        };

        if semver::Version::parse(json.geode.trim_start_matches("v")).is_err() {
            return Err(ApiError::BadRequest(format!("Invalid geode version semver {}", json.geode)));
        };

        Mod::create(json, pool).await?;
        ModVersion::create_from_json(json, pool).await?;
        Ok(())
    }

    pub async fn new_version(json: &ModJson, pool: &mut PgConnection) -> Result<(), ApiError> {
        let result = sqlx::query!("SELECT latest_version, validated, about, changelog FROM mods WHERE id = $1", json.id)
            .fetch_optional(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
        let result = match result {
            Some(r) => r,
            None => return Err(ApiError::NotFound(format!("Mod {} doesn't exist", &json.id)))
        };
        if !result.validated {
            return Err(ApiError::BadRequest("Cannot update an unverified mod. Please contact the Geode team for more details.".into()));
        }
        let version = semver::Version::parse(result.latest_version.trim_start_matches("v")).unwrap();
        let new_version = match semver::Version::parse(json.version.trim_start_matches("v")) {
            Ok(v) => v,
            Err(_) => return Err(ApiError::BadRequest(format!("Invalid semver {}", json.version)))
        };
        if new_version.le(&version) {
            return Err(ApiError::BadRequest(format!("mod.json version {} is smaller / equal to latest mod version {}", json.version, result.latest_version)));
        }
        ModVersion::create_from_json(json, pool).await?;
        let result = sqlx::query!(
            "UPDATE mods 
            SET latest_version = $1, changelog = $2, about = $3
            WHERE id = $4", json.version, json.changelog, json.about, json.id)
            .execute(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
        if result.rows_affected() == 0 {
            log::error!("{:?}", result);
            return Err(ApiError::DbError);
        }
        Ok(())
    }

    async fn create(json: &ModJson, pool: &mut PgConnection) -> Result<(), ApiError> {
        let res = sqlx::query!("SELECT id FROM mods WHERE id = $1", json.id)
            .fetch_optional(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
        if !res.is_none() {
            return Err(ApiError::BadRequest(format!("Mod {} already exists, consider creating a new version", json.id)));
        }
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO mods (");
        if json.repository.is_some() {
            query_builder.push("repository, ");
        }
        if json.changelog.is_some() {
            query_builder.push("changelog, ");
        }
        if json.about.is_some() {
            query_builder.push("about, ");
        }
        query_builder.push("id, latest_version, validated) VALUES (");
        let mut separated = query_builder.separated(", ");
        if json.repository.is_some() {
            separated.push_bind(json.repository.as_ref().unwrap());
        }
        if json.changelog.is_some() {
            separated.push_bind(&json.changelog);
        }
        if json.about.is_some() {
            separated.push_bind(&json.about);
        }
        separated.push_bind(&json.id);
        separated.push_bind(&json.version);
        separated.push_bind(false);
        separated.push_unseparated(")");
        
        let _ = query_builder
            .build()
            .execute(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
        Ok(())
    }
}

pub async fn download_geode_file(url: &str) -> Result<Cursor<Bytes>, ApiError> {
    let res = reqwest::get(url).await.or(Err(ApiError::BadRequest(String::from("Invalid URL"))))?;
    let content = Cursor::new(res.bytes().await.or(Err(ApiError::FilesystemError))?);
    Ok(content)
}