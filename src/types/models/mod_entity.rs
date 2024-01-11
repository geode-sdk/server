use serde::Serialize;
use sqlx::{PgConnection, QueryBuilder, Postgres};
use uuid::Uuid;
use std::io::Cursor;
use crate::types::{models::mod_version::ModVersion, api::{PaginatedData, ApiError}, mod_json::ModJson};
use log::info;

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

    // Not done yet
    // pub async fn get_one(id: String, pool: &mut PgConnection) -> Result<Mod, ApiError> {
    //     let record: Option<ModRecord> = sqlx::query_as!(ModRecord, 
    //         "SELECT
    //             id, repository, latest_version,

    //         FROM mods WHERE id = $1",
    //         id
    //     ).fetch_optional(&mut *pool)
    //         .await
    //         .or(Err(ApiError::DbError))?;
    //     let record = match record {
    //         Some(result) => result,
    //         None => return Err(ApiError::NotFound(format!("Mod {} not found!", id)))
    //     };
        
    // }

    pub async fn from_json(json: &ModJson, new_mod: bool, pool: &mut PgConnection) -> Result<(), ApiError> {
        Mod::create(json, pool).await?;
        Ok(())
    }

    async fn create(json: &ModJson, pool: &mut PgConnection) -> Result<(), ApiError> {
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
        
        let result = query_builder
            .build()
            .execute(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
        if result.rows_affected() == 0 {
            return Err(ApiError::DbError);
        }
        Ok(())
    }
}

pub async fn download_geode_file(url: &str) -> Result<String, ApiError> {
    let res = reqwest::get(url).await.or(Err(ApiError::BadRequest(String::from("Invalid URL"))))?;
    if !std::fs::metadata("/tmp/geode-index").is_ok() {
        std::fs::create_dir("/tmp/geode-index").or(Err(ApiError::FilesystemError))?;
    }
    let file_path = format!("/tmp/geode-index/{}.geode", Uuid::new_v4());

    let mut file = std::fs::File::create(&file_path).or(Err(ApiError::FilesystemError))?;
    let mut content = Cursor::new(res.bytes().await.or(Err(ApiError::FilesystemError))?);
    std::io::copy(&mut content, &mut file).or(Err(ApiError::FilesystemError))?;
    Ok(file_path)
}