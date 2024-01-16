use std::collections::HashMap;

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
pub enum VerPlatform {
    Android,
    Ios,
    Mac,
    Win
}

#[derive(sqlx::FromRow)]
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

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO mod_gd_versions (gd, platform) VALUES");
        for i in 1..json.len() - 1 {
            builder.push("(");
            let mut separated = builder.separated(", ");
            let current = &json[i];
            separated.push_bind(current.gd as GDVersionEnum);
            separated.push_bind(current.platform as VerPlatform);
            separated.push_unseparated(")");
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