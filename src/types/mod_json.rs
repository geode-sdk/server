use std::io::{Cursor, Read};

use actix_web::web::Bytes;
use semver::Version;
use serde::Deserialize;
use sqlx::PgConnection;
use std::io::BufReader;
use zip::read::ZipFile;

use super::{
    api::ApiError,
    models::{
        dependency::{DependencyCreate, DependencyImportance, ModVersionCompare},
        incompatibility::{IncompatibilityCreate, IncompatibilityImportance},
        mod_gd_version::{DetailedGDVersion, GDVersionEnum},
    },
};

#[derive(Debug, Deserialize)]
pub struct ModJson {
    pub geode: String,
    pub version: String,
    pub id: String,
    pub name: String,
    pub developer: String,
    pub description: Option<String>,
    pub repository: Option<String>,
    pub issues: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub settings: Option<serde_json::Value>,
    #[serde(default)]
    pub windows: bool,
    #[serde(default)]
    pub ios: bool,
    #[serde(default)]
    pub android32: bool,
    #[serde(default)]
    pub android64: bool,
    #[serde(default)]
    pub mac: bool,
    #[serde(default)]
    pub download_url: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default, rename = "early-load")]
    pub early_load: bool,
    #[serde(default)]
    pub api: bool,
    pub gd: ModJsonGDVersionType,
    pub about: Option<String>,
    pub changelog: Option<String>,
    pub dependencies: Option<Vec<ModJsonDependency>>,
    pub incompatibilities: Option<Vec<ModJsonIncompatibility>>,
}

#[derive(Deserialize, Debug)]
pub struct ModJsonDependency {
    pub id: String,
    pub version: String,
    pub importance: DependencyImportance,
    // This should throw a deprecated error
    pub required: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct ModJsonIncompatibility {
    pub id: String,
    pub version: String,
    pub importance: IncompatibilityImportance,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ModJsonGDVersionType {
    VersionStr(GDVersionEnum),
    VersionObj(DetailedGDVersion),
}

impl ModJson {
    pub fn from_zip(file: &mut Cursor<Bytes>, download_url: &str) -> Result<ModJson, ApiError> {
        let mut bytes: Vec<u8> = vec![];
        match file.read_to_end(&mut bytes) {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::FilesystemError);
            }
            Ok(b) => b,
        };
        let hash = sha256::digest(bytes);
        let reader = BufReader::new(file);
        let mut archive = match zip::ZipArchive::new(reader) {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::BadRequest(
                    "Couldn't unzip .geode file".to_string(),
                ));
            }
            Ok(a) => a,
        };
        let json_file = archive
            .by_name("mod.json")
            .or(Err(ApiError::BadRequest(String::from(
                "mod.json not found",
            ))))?;
        let mut json = match serde_json::from_reader::<ZipFile, ModJson>(json_file) {
            Ok(j) => j,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::BadRequest("Invalid mod.json".to_string()));
            }
        };
        json.version = json.version.trim_start_matches("v").to_string();
        json.hash = hash;
        json.download_url = download_url.to_string();

        if json.dependencies.is_some() {
            for i in json.dependencies.as_mut().unwrap() {
                if !validate_dependency_version_str(&i.version) {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid dependency version {} for mod {}",
                        i.version, i.id
                    )));
                }
                if i.required.is_some() {
                    return Err(ApiError::BadRequest(format!(
                        "'required' key for dependencies is deprecated! Found at dependency id {}.",
                        i.id
                    )));
                }
                i.version = i.version.trim_start_matches("v").to_string();
            }
        }
        if json.incompatibilities.is_some() {
            for i in json.incompatibilities.as_mut().unwrap() {
                if !validate_dependency_version_str(&i.version) {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid incompatibility version {} for mod {}",
                        i.version, i.id
                    )));
                }
                i.version = i.version.trim_start_matches("v").to_string();
            }
        }

        for i in 0..archive.len() {
            if let Some(mut file) = archive.by_index(i).ok() {
                if file.name().ends_with(".dll") {
                    json.windows = true;
                    continue;
                }
                if file.name().ends_with(".ios.dylib") {
                    json.ios = true;
                    continue;
                }
                if file.name().ends_with(".dylib") {
                    json.mac = true;
                    continue;
                }
                if file.name().ends_with(".android32.so") {
                    json.android32 = true;
                    continue;
                }
                if file.name().ends_with(".android64.so") {
                    json.android64 = true;
                    continue;
                }
                if file.name().eq("about.md") {
                    json.about = match parse_zip_entry_to_str(&mut file) {
                        Err(e) => {
                            log::error!("{}", e);
                            return Err(ApiError::InternalError);
                        }
                        Ok(r) => Some(r),
                    };
                }
                if file.name().eq("changelog.md") {
                    json.changelog = match parse_zip_entry_to_str(&mut file) {
                        Err(e) => {
                            log::error!("{}", e);
                            return Err(ApiError::InternalError);
                        }
                        Ok(r) => Some(r),
                    };
                }
            }
        }
        return Ok(json);
    }

    pub async fn query_dependencies(
        &self,
        pool: &mut PgConnection,
    ) -> Result<Vec<DependencyCreate>, ApiError> {
        let deps = match self.dependencies.as_ref() {
            None => return Err(ApiError::InternalError),
            Some(d) => d,
        };

        if deps.is_empty() {
            return Err(ApiError::InternalError);
        }

        let mut ret: Vec<DependencyCreate> = vec![];

        // I am going to n+1 this, I am sorry, will optimize later
        for i in deps {
            let (dependency_ver, compare) = match split_version_and_compare(i.version.as_str()) {
                Err(_) => {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid semver {}",
                        i.version
                    )))
                }
                Ok((ver, compare)) => (ver, compare),
            };

            let versions = sqlx::query!(
                "SELECT id, version FROM mod_versions WHERE mod_id = $1 and validated = true",
                i.id
            )
            .fetch_all(&mut *pool)
            .await;
            let versions = match versions {
                Err(_) => return Err(ApiError::DbError),
                Ok(v) => v,
            };
            if versions.len() == 0 {
                return Err(ApiError::BadRequest(format!(
                    "Couldn't find dependency {} on the index",
                    i.id
                )));
            }
            let mut found = false;
            for j in versions {
                // This should never fail (I hope)
                let parsed = semver::Version::parse(&j.version).unwrap();
                if compare_versions(&parsed, &dependency_ver, &compare) {
                    ret.push(DependencyCreate {
                        dependency_id: j.id,
                        compare,
                        importance: i.importance,
                    });
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(ApiError::BadRequest(format!(
                    "Couldn't find dependency version that satisfies semver compare {}",
                    i.version
                )));
            }
        }

        Ok(ret)
    }

    pub async fn query_incompatibilities(
        &self,
        pool: &mut PgConnection,
    ) -> Result<Vec<IncompatibilityCreate>, ApiError> {
        let incompat = match self.incompatibilities.as_ref() {
            None => return Err(ApiError::InternalError),
            Some(d) => d,
        };

        if incompat.is_empty() {
            return Err(ApiError::InternalError);
        }
        let mut ret: Vec<_> = vec![];

        for i in incompat {
            let (ver, compare) = match split_version_and_compare(i.version.as_str()) {
                Err(_) => {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid semver {}",
                        i.version
                    )))
                }
                Ok((ver, compare)) => (ver, compare),
            };

            let versions = sqlx::query!(
                "SELECT id, version FROM mod_versions WHERE mod_id = $1",
                i.id
            )
            .fetch_all(&mut *pool)
            .await;
            let versions = match versions {
                Err(_) => return Err(ApiError::DbError),
                Ok(v) => v,
            };
            if versions.len() == 0 {
                return Err(ApiError::BadRequest(format!(
                    "Couldn't find incompatibility {} on the index",
                    i.id
                )));
            }
            let mut found = false;
            for j in versions {
                // This should never fail (I hope)
                let parsed = semver::Version::parse(&j.version).unwrap();
                if compare_versions(&parsed, &ver, &compare) {
                    ret.push(IncompatibilityCreate {
                        incompatibility_id: j.id,
                        compare,
                        importance: i.importance,
                    });
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(ApiError::BadRequest(format!(
                    "Couldn't find incompatibility version that satisfies semver compare {}",
                    i.version
                )));
            }
        }

        Ok(ret)
    }
}

fn compare_versions(
    v1: &semver::Version,
    v2: &semver::Version,
    compare: &ModVersionCompare,
) -> bool {
    match compare {
        ModVersionCompare::Exact => v1.eq(&v2),
        ModVersionCompare::Less => v1.lt(&v2),
        ModVersionCompare::LessEq => v1.le(&v2),
        ModVersionCompare::More => v1.gt(&v2),
        ModVersionCompare::MoreEq => v1.ge(&v2),
    }
}

fn parse_zip_entry_to_str(file: &mut ZipFile) -> Result<String, String> {
    let mut string: String = String::from("");
    match file.read_to_string(&mut string) {
        Ok(_) => Ok(string),
        Err(e) => {
            log::error!("{}", e);
            return Err(format!("Failed to parse {}", file.name()));
        }
    }
}

fn validate_dependency_version_str(ver: &str) -> bool {
    let mut copy = ver.to_string();
    if ver.starts_with("<=") {
        copy = copy.trim_start_matches("<=").to_string();
    } else if ver.starts_with(">=") {
        copy = copy.trim_start_matches(">=").to_string();
    } else if ver.starts_with("=") {
        copy = copy.trim_start_matches("=").to_string();
    } else if ver.starts_with("<") {
        copy = copy.trim_start_matches("<").to_string();
    } else if ver.starts_with(">") {
        copy = copy.trim_start_matches(">").to_string();
    }
    copy = copy.trim_start_matches("v").to_string();

    let result = semver::Version::parse(&copy);
    result.is_ok()
}

fn split_version_and_compare(ver: &str) -> Result<(Version, ModVersionCompare), ()> {
    let mut copy = ver.to_string();
    let mut compare = ModVersionCompare::MoreEq;
    if ver.starts_with("<=") {
        copy = copy.trim_start_matches("<=").to_string();
        compare = ModVersionCompare::LessEq;
    } else if ver.starts_with(">=") {
        copy = copy.trim_start_matches(">=").to_string();
        compare = ModVersionCompare::MoreEq;
    } else if ver.starts_with("=") {
        copy = copy.trim_start_matches("=").to_string();
        compare = ModVersionCompare::Exact;
    } else if ver.starts_with("<") {
        copy = copy.trim_start_matches("<").to_string();
        compare = ModVersionCompare::Less;
    } else if ver.starts_with(">") {
        copy = copy.trim_start_matches(">").to_string();
        compare = ModVersionCompare::More;
    }
    copy = copy.trim_start_matches("v").to_string();
    let ver = semver::Version::parse(&copy);
    match ver {
        Err(_) => Err(()),
        Ok(v) => Ok((v, compare)),
    }
}
