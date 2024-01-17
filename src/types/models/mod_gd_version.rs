use std::collections::{HashMap, hash_map::Entry};

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, QueryBuilder, Postgres};

use crate::types::{mod_json::ModJsonGDVersion, api::ApiError};

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy)]
#[sqlx(type_name = "gd_version")]
pub enum GDVersionEnum {
    #[serde(rename = "*")]
    #[sqlx(rename = "*")]
    All,
    #[serde(rename = "2.113")]
    #[sqlx(rename = "2.113")]
    GD2113,
    #[serde(rename = "2.200")]
    #[sqlx(rename = "2.200")]
    GD2200,
    #[serde(rename = "2.203")]
    #[sqlx(rename = "2.203")]
    GD2203,
}

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
#[sqlx(type_name = "gd_ver_platform", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum VerPlatform {
    Android,
    Ios,
    Mac,
    Win
}

#[derive(sqlx::FromRow, Clone, Copy, Debug, Serialize)]
pub struct ModGDVersion {
    id: i32,
    mod_id: i32,
    gd: GDVersionEnum,
    platform: VerPlatform
}

impl ModGDVersion {
    pub async fn create_from_json(json: Vec<ModJsonGDVersion>, mod_version_id: i32, pool: &mut PgConnection) -> Result<(), ApiError> {
        if json.len() == 0 {
            return Err(ApiError::BadRequest("mod.json gd version array has no elements".to_string()));
        }
        match check_for_duplicate_platforms(&json) {
            Err(e) => return Err(ApiError::BadRequest(e)),
            Ok(_) => ()
        };

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO mod_gd_versions (gd, platform, mod_id) VALUES ");
        let mut i = 0;
        for current in json.iter() {
            builder.push("(");
            let mut separated = builder.separated(", ");
            separated.push_bind(current.gd as GDVersionEnum);
            separated.push_bind(current.platform as VerPlatform);
            separated.push_bind(mod_version_id);
            separated.push_unseparated(")");
            i += 1;
            if i != json.len() {
                separated.push_unseparated(", ");
            }
        }

        let result = builder.
            build()
            .execute(&mut *pool)
            .await;
        match result {
            Err(e) => {
                log::error!("{:?}", e);
                return Err(ApiError::DbError);
            },
            Ok(_) => ()
        };

        Ok(())
    }

    pub async fn create_for_all_platforms(version: GDVersionEnum, mod_version_id: i32, pool: &mut PgConnection) -> Result<(), ApiError> {
        let platforms = sqlx::query!("SELECT ios, android32, android64, windows, mac FROM mod_versions WHERE id = $1", mod_version_id)
            .fetch_one(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
        let mut platforms_arg: Vec<ModJsonGDVersion> = vec![];
        if platforms.android32 || platforms.android64 {
            platforms_arg.push(ModJsonGDVersion { gd: version, platform: VerPlatform::Android })
        }
        if platforms.windows {
            platforms_arg.push(ModJsonGDVersion { gd: version, platform: VerPlatform::Win})
        }
        if platforms.mac {
            platforms_arg.push(ModJsonGDVersion { gd: version, platform: VerPlatform::Mac})
        }
        if platforms.ios {
            platforms_arg.push(ModJsonGDVersion { gd: version, platform: VerPlatform::Ios})
        }
        ModGDVersion::create_from_json(platforms_arg, mod_version_id, pool).await?;
        Ok(())
    }

    // to be used for GET mods/{id}/version/{version}
    pub async fn get_for_mod_version(id: i32, pool: &mut PgConnection) -> Result<Vec<ModGDVersion>, ApiError> {
        let result = sqlx::query_as!(ModGDVersion, r#"SELECT mgv.id, mgv.mod_id, mgv.gd AS "gd: _", mgv.platform as "platform: _" FROM mod_gd_versions mgv WHERE mgv.mod_id = $1"#, id)
            .fetch_all(&mut *pool)
            .await;
        let result = match result {
            Err(e) => {
                log::info!("{:?}", e);
                return Err(ApiError::DbError)
            },
            Ok(r) => r
        };

        Ok(result)
    }

    pub async fn get_for_mod_versions(versions: Vec<i32>, pool: &mut PgConnection) -> Result<HashMap<i32, Vec<ModGDVersion>>, ApiError> {
        if versions.len() == 0 {
            return Err(ApiError::DbError);
        }
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM mod_gd_versions WHERE mod_id IN (");
        let mut separated = builder.separated(", ");
        for i in versions {
            separated.push_bind(i);
        }
        separated.push_unseparated(")");
        log::info!("{}", builder.sql());

        let result = builder.build_query_as::<ModGDVersion>()
            .fetch_all(&mut *pool)
            .await;
        let result = match result {
            Err(e) => {
                log::info!("{:?}", e);
                return Err(ApiError::DbError);
            },
            Ok(r) => r
        };

        let mut ret: HashMap<i32, Vec<ModGDVersion>> = HashMap::new();
        for i in result {
            match ret.entry(i.mod_id) {
                Entry::Vacant(e) => {
                    let vec: Vec<ModGDVersion> = vec![i];
                    e.insert(vec);
                },
                Entry::Occupied(mut e) => { e.get_mut().push(i) }
            }
        }

        Ok(ret)
    }
}

fn check_for_duplicate_platforms(versions: &Vec<ModJsonGDVersion>) -> Result<(), String> {
    let mut found: HashMap<VerPlatform, GDVersionEnum> = HashMap::new();
    for i in versions {
        match found.get(&i.platform) {
            Some(_) => return Err("Duplicated platforms detected in mod.json gd key".to_string()),
            None => found.insert(i.platform, i.gd)
        };
    }
    Ok(())
}