use std::{collections::HashMap, str::FromStr};

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use crate::{database::DatabaseError, types::mod_json::ModJson};

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, Hash, PartialEq, Eq)]
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
    #[serde(rename = "2.2074")]
    #[sqlx(rename = "2.2074")]
    GD22074,
    #[serde(rename = "2.208")]
    #[sqlx(rename = "2.208")]
    GD2208,
    #[serde(rename = "2.2081")]
    #[sqlx(rename = "2.2081")]
    GD22081,
    #[serde(rename = "2.2082")]
    #[sqlx(rename = "2.2082")]
    GD22082,
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
            "2.2074" => Ok(GDVersionEnum::GD22074),
            "2.208" => Ok(GDVersionEnum::GD2208),
            "2.2081" => Ok(GDVersionEnum::GD22081),
            "2.2082" => Ok(GDVersionEnum::GD22082),
            _ => Err(()),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum PlatformParseError {
    #[error("Invalid platform {0}")]
    InvalidPlatform(String),
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
    pub fn parse_query_string(s: &str) -> Result<Vec<VerPlatform>, PlatformParseError> {
        let mut ret = vec![];

        for x in s.split(',') {
            let x = x.trim();
            if x.len() == 0 {
                continue;
            }
            let v = VerPlatform::from_str(x)
                .map_err(|_| PlatformParseError::InvalidPlatform(x.into()))?;

            match v {
                VerPlatform::Android => {
                    ret.push(VerPlatform::Android32);
                    ret.push(VerPlatform::Android64);
                }
                VerPlatform::Mac => {
                    ret.push(VerPlatform::MacArm);
                    ret.push(VerPlatform::MacIntel);
                }
                default => ret.push(default),
            }
        }

        Ok(ret)
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
    // to be used for GET mods/{id}/version/{version}
    pub async fn get_for_mod_version(
        id: i32,
        pool: &mut PgConnection,
    ) -> Result<DetailedGDVersion, DatabaseError> {
        let result = sqlx::query_as!(
            ModGDVersion,
            r#"
            SELECT mgv.id, mgv.mod_id, mgv.gd AS "gd: _", mgv.platform as "platform: _"
            FROM mod_gd_versions mgv
            WHERE mgv.mod_id = $1
            "#,
            id
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| {
            log::error!("Failed to fetch mod_gd_versions for mod_version {id}: {e}")
        })?;
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
        versions: &[i32],
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, DetailedGDVersion>, DatabaseError> {
        if versions.is_empty() {
            return Ok(HashMap::new());
        }

        let result = sqlx::query_as!(
            ModGDVersion,
            r#"SELECT
                id, mod_id, gd as "gd: _", platform as "platform: _"
            FROM mod_gd_versions
            WHERE mod_id = ANY($1)"#,
            versions
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| log::error!("Failed to fetch mod_gd_versions: {}", e))?;

        let mut ret: HashMap<i32, DetailedGDVersion> = HashMap::new();
        for i in result {
            let ver = ret.entry(i.mod_id).or_default();
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
        }

        Ok(ret)
    }
}
