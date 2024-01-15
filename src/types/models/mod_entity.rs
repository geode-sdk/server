use serde::Serialize;
use sqlx::{PgConnection, QueryBuilder, Postgres};
use uuid::Uuid;
use std::io::Cursor;
use crate::types::{models::mod_version::ModVersion, api::{PaginatedData, ApiError}, mod_json::ModJson};

#[derive(Serialize, Debug, sqlx::FromRow)]
pub struct Mod {
    pub id: String,
    pub repository: Option<String>,
    pub latest_version: String,
    pub validated: bool,
    pub versions: Vec<ModVersion>
}

#[derive(Debug)]
struct ModRecord {
    id: String,
    repository: Option<String>,
    latest_version: String,
    validated: bool,
}

#[derive(Debug)]
struct ModRecordWithVersions {
    id: String,
    repository: Option<String>,
    latest_version: String,
    validated: bool,
    version_id: i64,
    name: String,
    description: Option<String>,
    version: String,
    download_link: String,
    hash: String,
    geode_version: String,
    windows: bool,
    android32: bool,
    android64: bool,
    mac: bool,
    ios: bool,
    mod_id: String
}

impl Mod {
    pub async fn get_index(pool: &mut PgConnection, page: i64, per_page: i64, query: String) -> Result<PaginatedData<Mod>, ApiError> {
        let limit = per_page;
        let offset = (page - 1) * per_page;
        let query_string = format!("%{query}%");
        let records: Vec<ModRecord> = sqlx::query_as!(ModRecord, r#"SELECT * FROM mods WHERE validated = true AND id LIKE $1 LIMIT $2 OFFSET $3"#, query_string, limit, offset)
            .fetch_all(&mut *pool)
            .await.or(Err(ApiError::DbError))?;
        let count = sqlx::query_scalar!("SELECT COUNT(*) FROM mods WHERE validated = true")
            .fetch_one(&mut *pool)
            .await.or(Err(ApiError::DbError))?.unwrap_or(0);

        let ids: Vec<_> = records.iter().map(|x| x.id.as_str()).collect();
        let versions = ModVersion::get_versions_for_mods(pool, &ids).await?;

        let ret = records.into_iter().map(|x| {
            let version_vec = versions.get(&x.id).cloned().unwrap_or_default();
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
        let records: Vec<ModRecordWithVersions> = sqlx::query_as!(ModRecordWithVersions, 
            "SELECT
                m.id, m.repository, m.latest_version, m.validated,
                mv.id as version_id, mv.name, mv.description, mv.version, mv.download_link,
                mv.hash, mv.geode_version, mv.windows, mv.android32, mv.android64, mv.mac, mv.ios,
                mv.mod_id
            FROM mods m
            LEFT JOIN mod_versions mv ON m.id = mv.mod_id
            WHERE m.id = $1",
            id
        ).fetch_all(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
        if records.len() == 0 {
            return Ok(None);
        }
        let versions = records.iter().map(|x| {
            ModVersion {
                id: x.version_id,
                name: x.name.clone(),
                description: x.description.clone(),
                version: x.version.clone(),
                download_link: x.download_link.clone(),
                hash: x.hash.clone(),
                geode_version: x.geode_version.clone(),
                windows: x.windows,
                android32: x.android32,
                android64: x.android64,
                mac: x.mac,
                ios: x.ios,
                mod_id: x.mod_id.clone()
            }
        }).collect();
        let mod_entity = Mod {
            id: records[0].id.clone(),
            repository: records[0].repository.clone(),
            latest_version: records[0].latest_version.clone(),
            validated: records[0].validated,
            versions
        };
        Ok(Some(mod_entity))
    }

    pub async fn from_json(json: &ModJson, new_mod: bool, pool: &mut PgConnection) -> Result<(), ApiError> {
        if semver::Version::parse(json.version.trim_start_matches("v")).is_err() {
            return Err(ApiError::BadRequest(format!("Invalid mod version semver {}", json.version)));
        };

        if semver::Version::parse(json.geode.trim_start_matches("v")).is_err() {
            return Err(ApiError::BadRequest(format!("Invalid geode version semver {}", json.geode)));
        };

        if new_mod {
            Mod::create(json, pool).await?;
        }
        ModVersion::from_json(json, pool).await?;
        Ok(())
    }

    pub async fn new_version(json: &ModJson, pool: &mut PgConnection) -> Result<(), ApiError> {
        let result = sqlx::query!("SELECT latest_version, validated FROM mods WHERE id = $1", json.id)
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
        ModVersion::from_json(json, pool).await?;
        let result = sqlx::query!("UPDATE mods SET latest_version = $1 WHERE id = $2", json.version, json.id)
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
        query_builder.push("id, latest_version, validated) VALUES (");
        let mut separated = query_builder.separated(", ");
        if json.repository.is_some() {
            separated.push_bind(json.repository.as_ref().unwrap());
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

pub async fn download_geode_file(url: &str) -> Result<String, ApiError> {
    let res = reqwest::get(url).await.or(Err(ApiError::BadRequest(String::from("Invalid URL"))))?;
    if !tokio::fs::metadata("/tmp/geode-index").await.is_ok() {
        tokio::fs::create_dir("/tmp/geode-index").await.or(Err(ApiError::FilesystemError))?;
    }
    let file_path = format!("/tmp/geode-index/{}.geode", Uuid::new_v4());

    let mut file = tokio::fs::File::create(&file_path).await.or(Err(ApiError::FilesystemError))?;
    let mut content = Cursor::new(res.bytes().await.or(Err(ApiError::FilesystemError))?);
    tokio::io::copy(&mut content, &mut file).await.or(Err(ApiError::FilesystemError))?;
    Ok(file_path)
}