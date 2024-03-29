use std::{
    collections::{hash_map::Entry, HashMap},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, QueryBuilder};

use crate::types::{api::ApiError, mod_json::ModJson};

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
    #[serde(rename = "2.204")]
    #[sqlx(rename = "2.204")]
    GD2204,
    #[serde(rename = "2.205")]
    #[sqlx(rename = "2.205")]
    GD2205,
}

impl FromStr for GDVersionEnum {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "*" => Ok(GDVersionEnum::All),
            "2.113" => Ok(GDVersionEnum::GD2113),
            "2.200" => Ok(GDVersionEnum::GD2200),
            "2.204" => Ok(GDVersionEnum::GD2204),
            "2.205" => Ok(GDVersionEnum::GD2205),
            _ => Err(()),
        }
    }
}

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
#[sqlx(type_name = "gd_ver_platform", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum VerPlatform {
    #[sqlx(skip)]
    Android,
    #[serde(skip_deserializing)]
    Android32,
    #[serde(skip_deserializing)]
    Android64,
    Ios,
    Mac,
    Win,
}

impl FromStr for VerPlatform {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "android" => Ok(VerPlatform::Android),
            "android32" => Ok(VerPlatform::Android32),
            "android64" => Ok(VerPlatform::Android64),
            "ios" => Ok(VerPlatform::Ios),
            "mac" => Ok(VerPlatform::Mac),
            "win" => Ok(VerPlatform::Win),
            "windows" => Ok(VerPlatform::Win),
            "macos" => Ok(VerPlatform::Mac),
            _ => Err(()),
        }
    }
}

#[derive(sqlx::FromRow, Clone, Copy, Debug, Serialize)]
pub struct ModGDVersion {
    id: i32,
    mod_id: i32,
    gd: GDVersionEnum,
    platform: VerPlatform,
}

pub struct ModGDVersionCreate {
    pub gd: GDVersionEnum,
    pub platform: VerPlatform,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct DetailedGDVersion {
    pub win: Option<GDVersionEnum>,
    #[serde(skip_serializing)]
    pub android: Option<GDVersionEnum>,
    #[serde(skip_deserializing)]
    pub android32: Option<GDVersionEnum>,
    #[serde(skip_deserializing)]
    pub android64: Option<GDVersionEnum>,
    pub mac: Option<GDVersionEnum>,
    pub ios: Option<GDVersionEnum>,
}

impl DetailedGDVersion {
    pub fn to_create_payload(&self, json: &ModJson) -> Vec<ModGDVersionCreate> {
        let mut ret: Vec<_> = vec![];
        if self.android.is_some() {
            if json.android32 {
                ret.push(ModGDVersionCreate {
                    gd: self.android.unwrap(),
                    platform: VerPlatform::Android64,
                });
            }
            if json.android64 {
                ret.push(ModGDVersionCreate {
                    gd: self.android.unwrap(),
                    platform: VerPlatform::Android32,
                })
            }
        }
        if self.win.is_some() && json.windows {
            ret.push(ModGDVersionCreate {
                gd: self.win.unwrap(),
                platform: VerPlatform::Win,
            });
        }
        if self.mac.is_some() && json.mac {
            ret.push(ModGDVersionCreate {
                gd: self.mac.unwrap(),
                platform: VerPlatform::Mac,
            });
        }
        if self.ios.is_some() && json.ios {
            ret.push(ModGDVersionCreate {
                gd: self.ios.unwrap(),
                platform: VerPlatform::Ios,
            });
        }

        ret
    }
}

impl ModGDVersion {
    pub async fn create_from_json(
        json: Vec<ModGDVersionCreate>,
        mod_version_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if json.is_empty() {
            return Err(ApiError::BadRequest(
                "mod.json gd version array has no elements".to_string(),
            ));
        }

        if let Err(e) = check_for_duplicate_platforms(&json) {
            return Err(ApiError::BadRequest(e));
        }

        let mut builder: QueryBuilder<Postgres> =
            QueryBuilder::new("INSERT INTO mod_gd_versions (gd, platform, mod_id) VALUES ");
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

        if let Err(e) = builder.build().execute(&mut *pool).await {
            log::error!("{:?}", e);
            return Err(ApiError::DbError);
        }

        Ok(())
    }

    pub async fn create_for_all_platforms(
        json: &ModJson,
        version: GDVersionEnum,
        version_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let mut platforms_arg: Vec<ModGDVersionCreate> = vec![];
        if json.android32 {
            platforms_arg.push(ModGDVersionCreate {
                gd: version,
                platform: VerPlatform::Android32,
            })
        }
        if json.android64 {
            platforms_arg.push(ModGDVersionCreate {
                gd: version,
                platform: VerPlatform::Android64,
            })
        }
        if json.windows {
            platforms_arg.push(ModGDVersionCreate {
                gd: version,
                platform: VerPlatform::Win,
            })
        }
        if json.mac {
            platforms_arg.push(ModGDVersionCreate {
                gd: version,
                platform: VerPlatform::Mac,
            })
        }
        if json.ios {
            platforms_arg.push(ModGDVersionCreate {
                gd: version,
                platform: VerPlatform::Ios,
            })
        }
        ModGDVersion::create_from_json(platforms_arg, version_id, pool).await?;
        Ok(())
    }

    // to be used for GET mods/{id}/version/{version}
    pub async fn get_for_mod_version(
        id: i32,
        pool: &mut PgConnection,
    ) -> Result<DetailedGDVersion, ApiError> {
        let result = sqlx::query_as!(ModGDVersion, r#"SELECT mgv.id, mgv.mod_id, mgv.gd AS "gd: _", mgv.platform as "platform: _" FROM mod_gd_versions mgv WHERE mgv.mod_id = $1"#, id)
            .fetch_all(&mut *pool)
            .await;
        let result: Vec<ModGDVersion> = match result {
            Err(e) => {
                log::info!("{:?}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };
        let mut ret = DetailedGDVersion {
            win: None,
            mac: None,
            android: None,
            ios: None,
            android32: None,
            android64: None,
        };
        for i in result {
            match i.platform {
                VerPlatform::Android32 => ret.android32 = Some(i.gd),
                VerPlatform::Android64 => ret.android64 = Some(i.gd),
                VerPlatform::Android => {
                    ret.android32 = Some(i.gd);
                    ret.android64 = Some(i.gd);
                }
                VerPlatform::Win => ret.win = Some(i.gd),
                VerPlatform::Mac => ret.mac = Some(i.gd),
                VerPlatform::Ios => ret.ios = Some(i.gd),
            }
        }

        Ok(ret)
    }

    // hello
    pub async fn get_for_mod_versions(
        versions: &Vec<i32>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, DetailedGDVersion>, ApiError> {
        if versions.is_empty() {
            return Err(ApiError::DbError);
        }
        let mut builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT * FROM mod_gd_versions WHERE mod_id IN (");
        let mut separated = builder.separated(", ");
        for i in versions {
            separated.push_bind(i);
        }
        separated.push_unseparated(")");

        let result = builder
            .build_query_as::<ModGDVersion>()
            .fetch_all(&mut *pool)
            .await;
        let result = match result {
            Err(e) => {
                log::info!("{:?}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        let mut ret: HashMap<i32, DetailedGDVersion> = HashMap::new();
        for i in result {
            match ret.entry(i.mod_id) {
                Entry::Vacant(e) => {
                    let mut ver = DetailedGDVersion::default();
                    match i.platform {
                        VerPlatform::Android => {
                            ver.android32 = Some(i.gd);
                            ver.android64 = Some(i.gd);
                        }
                        VerPlatform::Android32 => ver.android32 = Some(i.gd),
                        VerPlatform::Android64 => ver.android64 = Some(i.gd),
                        VerPlatform::Mac => ver.mac = Some(i.gd),
                        VerPlatform::Ios => ver.ios = Some(i.gd),
                        VerPlatform::Win => ver.win = Some(i.gd),
                    }
                    e.insert(ver);
                }
                Entry::Occupied(mut e) => match i.platform {
                    VerPlatform::Android => {
                        e.get_mut().android32 = Some(i.gd);
                        e.get_mut().android64 = Some(i.gd);
                    }
                    VerPlatform::Android32 => e.get_mut().android32 = Some(i.gd),
                    VerPlatform::Android64 => e.get_mut().android64 = Some(i.gd),
                    VerPlatform::Mac => e.get_mut().mac = Some(i.gd),
                    VerPlatform::Ios => e.get_mut().ios = Some(i.gd),
                    VerPlatform::Win => e.get_mut().win = Some(i.gd),
                },
            }
        }

        Ok(ret)
    }
}

fn check_for_duplicate_platforms(versions: &Vec<ModGDVersionCreate>) -> Result<(), String> {
    let mut found: HashMap<VerPlatform, GDVersionEnum> = HashMap::new();
    for i in versions {
        match found.get(&i.platform) {
            Some(_) => return Err("Duplicated platforms detected in mod.json gd key".to_string()),
            None => found.insert(i.platform, i.gd),
        };
    }
    Ok(())
}
