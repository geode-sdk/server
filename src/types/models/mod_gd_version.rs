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
    #[serde(rename = "2.206")]
    #[sqlx(rename = "2.206")]
    GD2206,
    #[serde(rename = "2.207")]
    #[sqlx(rename = "2.207")]
    GD2207,
    #[serde(rename = "2.2071")]
    #[sqlx(rename = "2.2071")]
    GD22071,
    #[serde(rename = "2.2072")]
    #[sqlx(rename = "2.2072")]
    GD22072,
    #[serde(rename = "2.2073")]
    #[sqlx(rename = "2.2073")]
    GD22073,
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
            "2.206" => Ok(GDVersionEnum::GD2206),
            "2.207" => Ok(GDVersionEnum::GD2207),
            "2.2071" => Ok(GDVersionEnum::GD22071),
            "2.2072" => Ok(GDVersionEnum::GD22072),
            "2.2073" => Ok(GDVersionEnum::GD22073),
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
    Android32,
    Android64,
    Ios,
    #[sqlx(skip)]
    Mac,
    #[sqlx(rename = "mac-arm")]
    #[serde(rename = "mac-arm")]
    MacArm,
    #[sqlx(rename = "mac-intel")]
    #[serde(rename = "mac-intel")]
    MacIntel,
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
            "mac-arm" => Ok(VerPlatform::MacArm),
            "mac-intel" => Ok(VerPlatform::MacIntel),
            "win" => Ok(VerPlatform::Win),
            "windows" => Ok(VerPlatform::Win),
            "macos" => Ok(VerPlatform::Mac),
            _ => Err(()),
        }
    }
}

impl VerPlatform {
    pub fn parse_query_string(s: &str) -> Vec<VerPlatform> {
        let mut ret = vec![];
        if s.is_empty() {
            return ret;
        }

        for x in s.split(',') {
            match VerPlatform::from_str(x) {
                Ok(v) => {
                    if v == VerPlatform::Android {
                        ret.push(VerPlatform::Android32);
                        ret.push(VerPlatform::Android64);
                    } else {
                        ret.push(v);
                    }
                }
                Err(_) => {
                    log::error!("invalid platform {}", x);
                }
            }
        }
        ret
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
    #[serde(skip_serializing)]
    pub mac: Option<GDVersionEnum>,
    #[serde(rename = "mac-intel")]
    pub mac_intel: Option<GDVersionEnum>,
    #[serde(rename = "mac-arm")]
    pub mac_arm: Option<GDVersionEnum>,
    pub ios: Option<GDVersionEnum>,
}

impl DetailedGDVersion {
    pub fn to_create_payload(&self, json: &ModJson) -> Vec<ModGDVersionCreate> {
        let mut ret: Vec<_> = vec![];
        if self.android.is_some() {
            if json.android32 {
                ret.push(ModGDVersionCreate {
                    gd: self.android.unwrap(),
                    platform: VerPlatform::Android32,
                });
            }
            if json.android64 {
                ret.push(ModGDVersionCreate {
                    gd: self.android.unwrap(),
                    platform: VerPlatform::Android64,
                })
            }
        }
        if self.win.is_some() && json.windows {
            ret.push(ModGDVersionCreate {
                gd: self.win.unwrap(),
                platform: VerPlatform::Win,
            });
        }
        if self.mac.is_some() {
            if json.mac_arm {
                ret.push(ModGDVersionCreate {
                    gd: self.mac.unwrap(),
                    platform: VerPlatform::MacArm,
                })
            }
            if json.mac_intel {
                ret.push(ModGDVersionCreate {
                    gd: self.mac.unwrap(),
                    platform: VerPlatform::MacIntel,
                })
            }
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
            mac_intel: None,
            mac_arm: None,
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
                VerPlatform::MacArm => ret.mac_arm = Some(i.gd),
                VerPlatform::MacIntel => ret.mac_intel = Some(i.gd),
                VerPlatform::Win => ret.win = Some(i.gd),
                VerPlatform::Mac => {
                    ret.mac_arm = Some(i.gd);
                    ret.mac_intel = Some(i.gd);
                }
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
                        VerPlatform::MacArm => ver.mac_arm = Some(i.gd),
                        VerPlatform::MacIntel => ver.mac_intel = Some(i.gd),
                        VerPlatform::Mac => {
                            ver.mac_arm = Some(i.gd);
                            ver.mac_intel = Some(i.gd);
                        }
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
                    VerPlatform::Mac => {
                        e.get_mut().mac_arm = Some(i.gd);
                        e.get_mut().mac_intel = Some(i.gd);
                    }
                    VerPlatform::MacArm => e.get_mut().mac_arm = Some(i.gd),
                    VerPlatform::MacIntel => e.get_mut().mac_intel = Some(i.gd),
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
