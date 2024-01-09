use std::{collections::{HashMap, hash_map::Entry}, vec};

use serde::Serialize;
use sqlx::{PgConnection, QueryBuilder, Postgres};

use crate::Error;

#[derive(Serialize, Debug, sqlx::FromRow, Clone)]
pub struct ModVersion {
    pub id: i64,
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
    pub mod_id: String 
}

#[derive(sqlx::Type, Debug)]
#[sqlx(type_name = "mood", rename_all = "lowercase")]
pub enum ModImportance {
    Required,
    Recommended,
    Suggested
}

#[derive(sqlx::FromRow)]
struct ModVersionRecord {
    id: i64,
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

impl ModVersion {
    pub async fn get_versions_for_mods(pool: &mut PgConnection, ids: Vec<&str>) -> Result<HashMap<String, Vec<ModVersion>>, Error> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT * FROM mod_versions WHERE mod_id IN ("
        );
        let mut separated = query_builder.separated(",");
        for id in ids.iter() {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        let records = query_builder.build_query_as::<ModVersionRecord>().fetch_all(pool).await.or(Err(Error::DbError))?;
        let mut ret: HashMap<String, Vec<ModVersion>> = HashMap::new();
        
        for x in records.iter() {
            let mod_id = x.mod_id.clone();
            let version = ModVersion {
                id: x.id,
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
            };
            match ret.entry(mod_id) {
                Entry::Vacant(e) => {
                    let vector: Vec<ModVersion> = vec![version];
                    e.insert(vector);
                },
                Entry::Occupied(mut e) => { e.get_mut().push(version) }
            }
        }
        return Ok(ret);
    }
}