use std::collections::HashMap;
use std::io::{Cursor, Read};

use actix_web::web::Bytes;
use regex::Regex;
use reqwest::Url;
use semver::Version;
use serde::Deserialize;
use zip::read::ZipFile;

use crate::mod_zip::{self, ModZipError};

use super::models::{
    dependency::{DependencyCreate, DependencyImportance, ModVersionCompare},
    incompatibility::{IncompatibilityCreate, IncompatibilityImportance},
    mod_gd_version::DetailedGDVersion,
};

#[derive(Debug, Deserialize)]
pub struct ModJson {
    pub geode: String,
    pub version: String,
    pub id: String,
    pub name: String,
    pub developer: Option<String>,
    pub developers: Option<Vec<String>>,
    pub description: Option<String>,
    pub repository: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_deserializing)]
    pub windows: bool,
    #[serde(default, skip_deserializing)]
    pub ios: bool,
    #[serde(default, skip_deserializing)]
    pub android32: bool,
    #[serde(default, skip_deserializing)]
    pub android64: bool,
    #[serde(default, skip_deserializing)]
    pub mac_intel: bool,
    #[serde(default, skip_deserializing)]
    pub mac_arm: bool,
    #[serde(default, skip_deserializing)]
    pub download_url: String,
    #[serde(default, skip_deserializing)]
    pub hash: String,
    #[serde(default, rename = "early-load")]
    pub early_load: bool,
    pub api: Option<serde_json::Value>,
    pub gd: DetailedGDVersion,
    #[serde(skip_deserializing, skip_serializing)]
    pub logo: Vec<u8>,
    pub about: Option<String>,
    pub changelog: Option<String>,
    pub dependencies: Option<ModJsonDependencies>,
    pub incompatibilities: Option<ModJsonIncompatibilities>,
    pub links: Option<ModJsonLinks>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModJsonLinks {
    pub community: Option<String>,
    pub homepage: Option<String>,
    pub source: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ModJsonDependencies {
    Old(Vec<OldModJsonDependency>),
    New(HashMap<String, ModJsonDependencyType>),
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ModJsonDependencyType {
    Version(String),
    Detailed(ModJsonDependency),
}

#[derive(Deserialize, Debug)]
pub struct ModJsonDependencyOnlyVersion {
    version: String,
}

#[derive(Deserialize, Debug)]
pub struct ModJsonDependencyWithImportance {
    version: String,
    importance: DependencyImportance,
}

#[derive(Deserialize, Debug)]
pub struct ModJsonDependencyWithRequired {
    version: String,
    required: bool,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ModJsonDependency {
    WithImportance(ModJsonDependencyWithImportance),
    WithRequired(ModJsonDependencyWithRequired),
    OnlyVersion(ModJsonDependencyOnlyVersion),
}

#[derive(Deserialize, Debug)]
pub struct OldModJsonDependency {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub importance: DependencyImportance,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ModJsonIncompatibilities {
    Old(Vec<OldModJsonIncompatibility>),
    New(HashMap<String, ModJsonIncompatibilityType>),
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ModJsonIncompatibilityType {
    Version(String),
    Detailed(ModJsonIncompatibility),
}

#[derive(Deserialize, Debug)]
pub struct ModJsonIncompatibilityOnlyVersion {
    version: String,
}

#[derive(Deserialize, Debug)]
pub struct ModJsonIncompatibilityWithImportance {
    version: String,
    importance: IncompatibilityImportance,
}

#[derive(Deserialize, Debug)]
pub struct ModJsonIncompatibilityWithBreaking {
    version: String,
    breaking: bool,
}

#[derive(Deserialize, Debug)]
pub enum ModJsonIncompatibility {
    WithImportance(ModJsonIncompatibilityWithImportance),
    WithBreaking(ModJsonIncompatibilityWithBreaking),
    OnlyVersion(ModJsonIncompatibilityOnlyVersion),
}

#[derive(Deserialize, Debug)]
pub struct OldModJsonIncompatibility {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub importance: IncompatibilityImportance,
}

impl ModJson {
    pub fn from_zip(
        file: Bytes,
        download_url: &str,
        store_image: bool,
    ) -> Result<ModJson, ModZipError> {
        let slice: &[u8] = &file;
        let hash = sha256::digest(slice);
        let mut archive = mod_zip::bytes_to_ziparchive(file)?;

        let json_file = archive
            .by_name("mod.json")
            .or(Err(ModZipError::InvalidModJson(
                "No mod.json found in .geode file".into(),
            )))?;

        let mut json = serde_json::from_reader::<ZipFile<Cursor<Bytes>>, ModJson>(json_file)
            .inspect_err(|e| log::error!("Failed to parse mod.json: {e}"))?;

        json.version = json.version.trim_start_matches('v').to_string();
        json.hash = hash;
        json.download_url = parse_download_url(download_url);

        for i in 0..archive.len() {
            if let Ok(mut file) = archive.by_index(i) {
                if file.name().ends_with(".dll") {
                    json.windows = true;
                } else if file.name().ends_with(".ios.dylib") {
                    json.ios = true;
                } else if file.name().ends_with(".dylib") {
                    let (arm, intel) = check_mac_binary(&mut file)?;
                    json.mac_arm = arm;
                    json.mac_intel = intel;
                } else if file.name().ends_with(".android32.so") {
                    json.android32 = true;
                } else if file.name().ends_with(".android64.so") {
                    json.android64 = true;
                } else if file.name().eq("about.md") {
                    json.about = Some(
                        parse_zip_entry_to_str(&mut file)
                            .inspect_err(|e| log::error!("Failed to parse about.md for mod: {e}"))
                            .map_err(|e| {
                                ModZipError::InvalidModJson(format!("Failed to read about.md: {e}"))
                            })?,
                    );
                } else if file.name().eq("changelog.md") {
                    json.changelog = Some(
                        parse_zip_entry_to_str(&mut file)
                            .inspect_err(|e| log::error!("Failed to parse changelog.md: {e}"))
                            .map_err(|e| {
                                ModZipError::InvalidModJson(format!(
                                    "Failed to read changelog.md: {e}"
                                ))
                            })?,
                    );
                } else if file.name() == "logo.png" {
                    if store_image {
                        json.logo = mod_zip::extract_mod_logo(&mut file)?;
                    } else {
                        mod_zip::validate_mod_logo(&mut file)?;
                    }
                }
            }
        }
        Ok(json)
    }

    pub fn prepare_dependencies_for_create(&self) -> Result<Vec<DependencyCreate>, ModZipError> {
        let deps = match self.dependencies.as_ref() {
            None => return Ok(vec![]),
            Some(d) => d,
        };

        match deps {
            ModJsonDependencies::Old(deps) => {
                if deps.is_empty() {
                    return Ok(vec![]);
                }

                let mut ret: Vec<DependencyCreate> = Vec::with_capacity(deps.len());

                for i in deps {
                    if i.version == "*" {
                        ret.push(DependencyCreate {
                            dependency_id: i.id.clone(),
                            version: "*".to_string(),
                            compare: ModVersionCompare::MoreEq,
                            importance: i.importance,
                        });
                        continue;
                    }
                    let (dependency_ver, compare) = split_version_and_compare(&(i.version))
                        .map_err(|_| {
                            ModZipError::InvalidModJson(format!("Invalid semver {}", i.version))
                        })?;
                    ret.push(DependencyCreate {
                        dependency_id: i.id.clone(),
                        version: dependency_ver.to_string(),
                        compare,
                        importance: i.importance,
                    });
                }
                Ok(ret)
            }
            ModJsonDependencies::New(deps) => {
                if deps.is_empty() {
                    return Ok(vec![]);
                }

                let mut ret: Vec<DependencyCreate> = Vec::with_capacity(deps.len());

                for (id, dep) in deps {
                    match dep {
                        ModJsonDependencyType::Version(version) => {
                            let (dependency_ver, compare) = split_version_and_compare(version)
                                .map_err(|_| {
                                    ModZipError::InvalidModJson(format!(
                                        "Invalid semver {}",
                                        version
                                    ))
                                })?;
                            ret.push(DependencyCreate {
                                dependency_id: id.clone(),
                                version: dependency_ver.to_string(),
                                compare,
                                importance: DependencyImportance::default(),
                            });
                        }
                        ModJsonDependencyType::Detailed(detailed) => {
                            let (dep_ver, importance) = match detailed {
                                ModJsonDependency::OnlyVersion(data) => {
                                    (&data.version, DependencyImportance::default())
                                }
                                ModJsonDependency::WithRequired(data) => (
                                    &data.version,
                                    match data.required {
                                        true => DependencyImportance::Required,
                                        false => DependencyImportance::Recommended,
                                    },
                                ),
                                ModJsonDependency::WithImportance(data) => {
                                    (&data.version, data.importance)
                                }
                            };
                            let (dependency_ver, compare) = split_version_and_compare(dep_ver)
                                .map_err(|_| {
                                    ModZipError::InvalidModJson(format!(
                                        "Invalid semver {}",
                                        dep_ver
                                    ))
                                })?;
                            ret.push(DependencyCreate {
                                dependency_id: id.clone(),
                                version: dependency_ver.to_string(),
                                compare,
                                importance,
                            });
                        }
                    }
                }
                Ok(ret)
            }
        }
    }

    pub fn prepare_incompatibilities_for_create(
        &self,
    ) -> Result<Vec<IncompatibilityCreate>, ModZipError> {
        let incompat = match self.incompatibilities.as_ref() {
            None => return Ok(vec![]),
            Some(d) => d,
        };

        match incompat {
            ModJsonIncompatibilities::Old(vec) => {
                if vec.is_empty() {
                    return Ok(vec![]);
                }

                let mut ret: Vec<IncompatibilityCreate> = Vec::with_capacity(vec.len());

                for i in vec {
                    if i.version == "*" {
                        ret.push(IncompatibilityCreate {
                            incompatibility_id: i.id.clone(),
                            version: "*".to_string(),
                            compare: ModVersionCompare::MoreEq,
                            importance: i.importance,
                        });
                        continue;
                    }

                    let (ver, compare) = split_version_and_compare(&(i.version)).map_err(|_| {
                        ModZipError::InvalidModJson(format!("Invalid semver: {}", i.version))
                    })?;
                    ret.push(IncompatibilityCreate {
                        incompatibility_id: i.id.clone(),
                        version: ver.to_string(),
                        compare,
                        importance: i.importance,
                    });
                }

                Ok(ret)
            }
            ModJsonIncompatibilities::New(map) => {
                if map.is_empty() {
                    return Ok(vec![]);
                }

                let mut ret: Vec<IncompatibilityCreate> = Vec::with_capacity(map.len());

                for (id, item) in map {
                    match item {
                        ModJsonIncompatibilityType::Version(version) => {
                            let (ver, compare) =
                                split_version_and_compare(version).map_err(|_| {
                                    ModZipError::InvalidModJson(format!(
                                        "Invalid semver {}",
                                        version
                                    ))
                                })?;
                            ret.push(IncompatibilityCreate {
                                incompatibility_id: id.clone(),
                                version: ver.to_string(),
                                compare,
                                importance: IncompatibilityImportance::default(),
                            });
                        }
                        ModJsonIncompatibilityType::Detailed(detailed) => {
                            let (inc_ver, importance) = match detailed {
                                ModJsonIncompatibility::OnlyVersion(data) => {
                                    (&data.version, IncompatibilityImportance::default())
                                }
                                ModJsonIncompatibility::WithBreaking(data) => (
                                    &data.version,
                                    match data.breaking {
                                        true => IncompatibilityImportance::Breaking,
                                        false => IncompatibilityImportance::Conflicting,
                                    },
                                ),
                                ModJsonIncompatibility::WithImportance(data) => {
                                    (&data.version, data.importance)
                                }
                            };
                            let (ver, compare) =
                                split_version_and_compare(inc_ver).map_err(|_| {
                                    ModZipError::InvalidModJson(format!(
                                        "Invalid semver {}",
                                        inc_ver
                                    ))
                                })?;
                            ret.push(IncompatibilityCreate {
                                incompatibility_id: id.clone(),
                                version: ver.to_string(),
                                compare,
                                importance,
                            });
                        }
                    }
                }

                Ok(ret)
            }
        }
    }

    pub fn validate(&self) -> Result<(), ModZipError> {
        let id_regex = Regex::new(r#"^[a-z0-9_\-]+\.[a-z0-9_\-]+$"#).unwrap();
        if !id_regex.is_match(&self.id) {
            return Err(ModZipError::InvalidModJson(format!(
                "Invalid mod id {} (lowercase and numbers only, needs to look like 'dev.mod')",
                self.id
            )));
        }

        if Version::parse(self.version.trim_start_matches('v')).is_err() {
            return Err(ModZipError::InvalidModJson(format!(
                "Invalid mod.json mod version: {}",
                self.version
            )));
        };

        if Version::parse(self.geode.trim_start_matches('v')).is_err() {
            return Err(ModZipError::InvalidModJson(format!(
                "Invalid mod.json geode version: {}",
                self.geode
            )));
        };

        if self.developer.is_none() && self.developers.is_none() {
            return Err(ModZipError::InvalidModJson(
                "No developer specified on mod.json".to_string(),
            ));
        }

        if self.id.len() > 64 {
            return Err(ModZipError::InvalidModJson(
                "Mod id too long (max 64 characters)".to_string(),
            ));
        }

        let v5: bool = {
            let geode = Version::parse(self.geode.trim_start_matches('v'))
                .map_err(|_| ModZipError::InvalidModJson("Invalid geode version".into()))?;
            geode.major >= 5
        };

        // Don't allow array syntax for geode >= v5
        if let Some(ModJsonDependencies::Old(_)) = &self.dependencies
            && v5
        {
            return Err(ModZipError::InvalidModJson(
                "Invalid dependencies key: should be an object".into(),
            ));
        }
        if let Some(ModJsonIncompatibilities::Old(_)) = &self.incompatibilities
            && v5
        {
            return Err(ModZipError::InvalidModJson(
                "Invalid incompatibilities key: should be an object".into(),
            ));
        }

        if let Some(l) = &self.links {
            if let Some(community) = &l.community {
                if let Err(e) = Url::parse(community) {
                    return Err(ModZipError::InvalidModJson(format!(
                        "Invalid community URL: {}. Reason: {}",
                        community, e
                    )));
                }
            }
            if let Some(homepage) = &l.homepage {
                if let Err(e) = Url::parse(homepage) {
                    return Err(ModZipError::InvalidModJson(format!(
                        "Invalid homepage URL: {}. Reason: {}",
                        homepage, e
                    )));
                }
            }
            if let Some(source) = &l.source {
                if let Err(e) = Url::parse(source) {
                    return Err(ModZipError::InvalidModJson(format!(
                        "Invalid source URL: {}. Reason: {}",
                        source, e
                    )));
                }
            }
        }
        Ok(())
    }
}

fn parse_zip_entry_to_str(file: &mut ZipFile<Cursor<Bytes>>) -> Result<String, String> {
    let mut string: String = String::from("");
    match file.read_to_string(&mut string) {
        Ok(_) => Ok(string),
        Err(e) => {
            log::error!("{}", e);
            Err(format!("Failed to parse {}", file.name()))
        }
    }
}

pub fn split_version_and_compare(ver: &str) -> Result<(Version, ModVersionCompare), ()> {
    let mut copy = ver.to_string();
    let mut compare = ModVersionCompare::MoreEq;
    if ver.starts_with("<=") {
        copy = copy.trim_start_matches("<=").to_string();
        compare = ModVersionCompare::LessEq;
    } else if ver.starts_with(">=") {
        copy = copy.trim_start_matches(">=").to_string();
        compare = ModVersionCompare::MoreEq;
    } else if ver.starts_with('=') {
        copy = copy.trim_start_matches('=').to_string();
        compare = ModVersionCompare::Exact;
    } else if ver.starts_with('<') {
        copy = copy.trim_start_matches('<').to_string();
        compare = ModVersionCompare::Less;
    } else if ver.starts_with('>') {
        copy = copy.trim_start_matches('>').to_string();
        compare = ModVersionCompare::More;
    }
    copy = copy.trim_start_matches('v').to_string();
    let ver = semver::Version::parse(&copy);
    match ver {
        Err(_) => Err(()),
        Ok(v) => Ok((v, compare)),
    }
}

fn parse_download_url(url: &str) -> String {
    String::from(url.trim_end_matches("\\/"))
}

fn check_mac_binary(file: &mut ZipFile<Cursor<Bytes>>) -> Result<(bool, bool), ModZipError> {
    // 12 bytes is all we need
    let mut bytes: Vec<u8> = vec![0; 12];
    file.read_exact(&mut bytes).map_err(|e| {
        log::error!("Failed to read MacOS binary: {}", e);
        ModZipError::InvalidBinaries(format!("Failed to read macOS binary: {e}"))
    })?;

    // Information taken from: https://www.jviotti.com/2021/07/23/a-deep-dive-on-macos-universal-binaries.html and some simple xxd fuckery

    // Universal
    // 4 Bytes for magic
    // 4 Bytes for num of architectures
    // Can be either ARM & x86 or only one
    // 0xCA 0xFE 0xBA 0xBE
    // Non-Universal
    // 0xCF 0xFA 0xED 0xFE

    let is_fat_arch = bytes[0] == 0xCA && bytes[1] == 0xFE && bytes[2] == 0xBA && bytes[3] == 0xBE;

    let is_fat_arch_64 =
        bytes[0] == 0xCA && bytes[1] == 0xFE && bytes[2] == 0xBA && bytes[3] == 0xBE;

    let is_single_platform =
        bytes[0] == 0xCF && bytes[1] == 0xFA && bytes[2] == 0xED && bytes[3] == 0xFE;

    if is_fat_arch || is_fat_arch_64 {
        let num_arches = bytes[7];
        if num_arches == 0x1 {
            let first = bytes[8];
            let second = bytes[11];
            if first == 0x1 && second == 0x7 {
                // intel - 0x01 0x00 0x00 0x07
                return Ok((false, true));
            } else if first == 0x1 && second == 0xC {
                // arm - 0x01 0x00 0x00 0x0C
                return Ok((true, false));
            } else {
                // probably invalid
                return Err(ModZipError::InvalidBinaries("Invalid macOS binary".into()));
            }
        } else if num_arches == 0x2 {
            return Ok((true, true));
        } else {
            return Err(ModZipError::InvalidBinaries("Invalid macOS binary".into()));
        }
    } else if is_single_platform {
        let first = bytes[4];
        let second = bytes[7];
        if first == 0x7 && second == 0x1 {
            // intel - 0x07 0x00 0x00 0x01
            return Ok((false, true));
        } else if first == 0xC && second == 0x1 {
            // arm - 0x0C 0x00 0x00 0x01
            return Ok((true, false));
        }
    }
    Err(ModZipError::InvalidBinaries("Invalid macOS binary".into()))
}
