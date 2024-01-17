use std::{collections::{HashMap, hash_map::Entry}, vec};

use serde::Serialize;
use sqlx::{PgConnection, QueryBuilder, Postgres, Row};

use crate::types::{api::ApiError, mod_json::{ModJson, ModJsonGDVersionType}};

use super::mod_gd_version::{ModGDVersion, GDVersionEnum};

#[derive(Serialize, Debug, sqlx::FromRow, Clone)]
pub struct ModVersion {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub download_link: String,
    pub hash: String,
    pub geode_version: String,
    pub windows: bool,
    pub android32: bool,
    pub android64: bool,
    pub mac: bool,
    pub ios: bool,
    pub early_load: bool,
    pub api: bool,
    pub mod_id: String,
    pub gd: Vec<ModGDVersion>
}

#[derive(sqlx::FromRow)]
struct ModVersionGetOne {
    id: i32,
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
    early_load: bool,
    api: bool,
    mod_id: String 
}

impl ModVersionGetOne {
    pub fn into_mod_version(&self) -> ModVersion {
        ModVersion {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            download_link: self.download_link.clone(),
            hash: self.hash.clone(),
            geode_version: self.geode_version.clone(),
            windows: self.windows,
            android32: self.android32,
            android64: self.android64,
            mac: self.mac,
            ios: self.ios,
            early_load: self.early_load,
            api: self.api,
            mod_id: self.mod_id.clone(),
            gd: vec![]
        }
    }
}

impl ModVersion {
    pub async fn get_latest_for_mods(pool: &mut PgConnection, ids: &[&str], gd: GDVersionEnum) -> Result<HashMap<String, Vec<ModVersion>>, ApiError> {
        if ids.is_empty() {
            return Ok(Default::default());
        }

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT mv.* FROM mod_versions mv 
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
            INNER JOIN mods m ON m.id = mv.mod_id
            WHERE mv.version = m.latest_version AND mgv.gd = "#
        );
        query_builder.push_bind(gd as GDVersionEnum);
        query_builder.push(" AND mv.mod_id IN (");
        let mut separated = query_builder.separated(",");
        for id in ids.iter() {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        let records = query_builder.build_query_as::<ModVersionGetOne>()
            .fetch_all(&mut *pool)
            .await;
        let records = match records {
            Err(e) => {
                log::info!("{:?}", e);
                return Err(ApiError::DbError);
            },
            Ok(r) => r
        };

        let mut ret: HashMap<String, Vec<ModVersion>> = HashMap::new();
        
        for x in records.iter() {
            let mod_id = x.mod_id.clone();
            let version = x.into_mod_version();
            match ret.entry(mod_id) {
                Entry::Vacant(e) => {
                    let vector: Vec<ModVersion> = vec![version];
                    e.insert(vector);
                },
                Entry::Occupied(mut e) => { e.get_mut().push(version) }
            }
        }
        Ok(ret)
    }

    pub async fn create_from_json(json: &ModJson, pool: &mut PgConnection) -> Result<(), ApiError> {
        // If someone finds a way to use macros with optional parameters you can impl it here
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO mod_versions (");
        if json.description.is_some() {
            builder.push("description, ");
        }
        builder.push("name, version, download_link, hash, geode_version, windows, android32, android64, mac, ios, early_load, api, mod_id) VALUES (");
        let mut separated = builder.separated(", ");
        if json.description.is_some() {
            separated.push_bind(&json.description);
        }
        separated.push_bind(&json.name);
        separated.push_bind(&json.version);
        separated.push_bind(&json.download_url);
        separated.push_bind(&json.hash);
        separated.push_bind(&json.geode);
        separated.push_bind(&json.windows);
        separated.push_bind(&json.android32);
        separated.push_bind(&json.android64);
        separated.push_bind(&json.mac);
        separated.push_bind(&json.ios);
        separated.push_bind(&json.early_load);
        separated.push_bind(&json.api);
        separated.push_bind(&json.id);
        separated.push_unseparated(") RETURNING id");
        let result = builder 
            .build()
            .fetch_one(&mut *pool)
            .await;
        let result = match result {
            Err(e) => {
                log::error!("{:?}", e);
                return Err(ApiError::DbError);
            },
            Ok(row) => row
        };
        let id = result.get::<i32, &str>("id");
        match json.gd.as_ref() {
            Some(gd) => match gd {
                ModJsonGDVersionType::VersionStr(ver) => ModGDVersion::create_for_all_platforms(*ver, id, pool).await?,
                ModJsonGDVersionType::VersionObj(vec) => ModGDVersion::create_from_json(vec.to_vec(), id, pool).await?
            },
            None => ()
        }
        Ok(())
    }

    // This will be used in GET /v1/mods/versions/{version}
    // pub async fn get_one(id: &str, version: &str, pool: &mut PgConnection) -> Result<ModVersion, ApiError> {
    //     let result = sqlx::query_as!(
    //         ModVersionRecord,
    //         "SELECT * FROM mod_versions WHERE mod_id = $1 AND version = $2",
    //         id, version
    //     ).fetch_optional(&mut *pool)
    //     .await
    //     .or(Err(ApiError::DbError))?;
        
    //     match result {
    //         Some(version) => Ok(version),
    //         None => Err(ApiError::NotFound(format!("Mod {}, version {} not found", id, version)))
    //     }
    // }
}