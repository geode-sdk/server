use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};

use actix_web::web::Bytes;
use image::{
    codecs::png::{PngDecoder, PngEncoder},
    DynamicImage, GenericImageView, ImageEncoder,
};
use regex::Regex;
use reqwest::Url;
use semver::Version;
use serde::Deserialize;
use std::io::BufReader;
use zip::read::ZipFile;

use super::{
    api::ApiError,
    models::{
        dependency::{DependencyCreate, DependencyImportance, ModVersionCompare},
        incompatibility::{IncompatibilityCreate, IncompatibilityImportance},
        mod_gd_version::DetailedGDVersion,
    },
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
    pub issues: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub settings: Option<serde_json::Value>,
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
pub struct ModJsonDependency {
    version: String,
    #[serde(default)]
    importance: DependencyImportance,
}

#[derive(Deserialize, Debug)]
pub struct OldModJsonDependency {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub importance: DependencyImportance,
    // This should throw a deprecated error
    pub required: Option<bool>,
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
pub struct ModJsonIncompatibility {
    version: String,
    #[serde(default)]
    pub importance: IncompatibilityImportance,
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
        file: &mut Cursor<Bytes>,
        download_url: &str,
        store_image: bool,
        max_size_mb: u32,
    ) -> Result<ModJson, ApiError> {
        let max_size_bytes = max_size_mb * 1_000_000;
        let mut bytes: Vec<u8> = vec![];
        let mut take = file.take(max_size_bytes as u64);
        match take.read_to_end(&mut bytes) {
            Err(e) => {
                log::error!("Failed to read bytes from {}: {}", download_url, e);
                return Err(ApiError::FilesystemError);
            }
            Ok(b) => b,
        };
        let hash = sha256::digest(bytes);
        let reader = BufReader::new(file);
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| {
            log::error!("Failed to create ZipArchive of mod: {}", e);
            ApiError::BadRequest("Failed to unzip .geode file".into())
        })?;
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
                    json.about = Some(parse_zip_entry_to_str(&mut file).map_err(|e| {
                        log::error!("Failed to parse about.md for mod: {}", e);
                        ApiError::InternalError
                    })?);
                } else if file.name().eq("changelog.md") {
                    json.changelog = Some(parse_zip_entry_to_str(&mut file).map_err(|e| {
                        log::error!("Failed to parse changelog.md for mod: {}", e);
                        ApiError::InternalError
                    })?);
                } else if file.name() == "logo.png" {
                    let bytes = validate_mod_logo(&mut file, store_image)?;
                    json.logo = bytes;
                }
            }
        }
        Ok(json)
    }

    pub fn prepare_dependencies_for_create(&self) -> Result<Vec<DependencyCreate>, ApiError> {
        let deps = match self.dependencies.as_ref() {
            None => return Ok(vec![]),
            Some(d) => d,
        };

        match deps {
            ModJsonDependencies::Old(deps) => {
                if deps.is_empty() {
                    return Ok(vec![]);
                }

                let mut ret: Vec<DependencyCreate> = vec![];
                ret.reserve(deps.len());

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
                            ApiError::BadRequest(format!("Invalid semver {}", i.version))
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

                let mut ret: Vec<DependencyCreate> = vec![];
                ret.reserve(deps.len());

                for (id, dep) in deps {
                    match dep {
                        ModJsonDependencyType::Version(version) => {
                            let (dependency_ver, compare) = split_version_and_compare(version)
                                .map_err(|_| {
                                    ApiError::BadRequest(format!("Invalid semver {}", version))
                                })?;
                            ret.push(DependencyCreate {
                                dependency_id: id.clone(),
                                version: dependency_ver.to_string(),
                                compare,
                                importance: DependencyImportance::default(),
                            });
                        }
                        ModJsonDependencyType::Detailed(detailed) => {
                            let (dependency_ver, compare) =
                                split_version_and_compare(&(detailed.version)).map_err(|_| {
                                    ApiError::BadRequest(format!(
                                        "Invalid semver {}",
                                        detailed.version
                                    ))
                                })?;
                            ret.push(DependencyCreate {
                                dependency_id: id.clone(),
                                version: dependency_ver.to_string(),
                                compare,
                                importance: detailed.importance,
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
    ) -> Result<Vec<IncompatibilityCreate>, ApiError> {
        let incompat = match self.incompatibilities.as_ref() {
            None => return Ok(vec![]),
            Some(d) => d,
        };

        match incompat {
            ModJsonIncompatibilities::Old(vec) => {
                if vec.is_empty() {
                    return Ok(vec![]);
                }

                let mut ret: Vec<IncompatibilityCreate> = vec![];
                ret.reserve(vec.len());

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
                        ApiError::BadRequest(format!("Invalid semver: {}", i.version))
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

                let mut ret: Vec<IncompatibilityCreate> = vec![];
                ret.reserve(map.len());

                for (id, item) in map {
                    match item {
                        ModJsonIncompatibilityType::Version(version) => {
                            let (ver, compare) =
                                split_version_and_compare(version).map_err(|_| {
                                    ApiError::BadRequest(format!("Invalid semver {}", version))
                                })?;
                            ret.push(IncompatibilityCreate {
                                incompatibility_id: id.clone(),
                                version: ver.to_string(),
                                compare,
                                importance: IncompatibilityImportance::default(),
                            });
                        }
                        ModJsonIncompatibilityType::Detailed(detailed) => {
                            let (ver, compare) = split_version_and_compare(&(detailed.version))
                                .map_err(|_| {
                                    ApiError::BadRequest(format!(
                                        "Invalid semver {}",
                                        detailed.version
                                    ))
                                })?;
                            ret.push(IncompatibilityCreate {
                                incompatibility_id: id.clone(),
                                version: ver.to_string(),
                                compare,
                                importance: detailed.importance,
                            });
                        }
                    }
                }

                Ok(ret)
            }
        }
    }

    pub fn validate(&self) -> Result<(), ApiError> {
        let id_regex = Regex::new(r#"^[a-z0-9_\-]+\.[a-z0-9_\-]+$"#).unwrap();
        if !id_regex.is_match(&self.id) {
            return Err(ApiError::BadRequest(format!(
                "Invalid mod id {} (lowercase and numbers only, needs to look like 'dev.mod')",
                self.id
            )));
        }

        if self.developer.is_none() && self.developers.is_none() {
            return Err(ApiError::BadRequest(
                "No developer specified on mod.json".to_string(),
            ));
        }

        if self.id.len() > 64 {
            return Err(ApiError::BadRequest(
                "Mod id too long (max 64 characters)".to_string(),
            ));
        }

        if let Some(l) = &self.links {
            if let Some(community) = &l.community {
                if let Err(e) = Url::parse(community) {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid community URL: {}. Reason: {}",
                        community, e
                    )));
                }
            }
            if let Some(homepage) = &l.homepage {
                if let Err(e) = Url::parse(homepage) {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid homepage URL: {}. Reason: {}",
                        homepage, e
                    )));
                }
            }
            if let Some(source) = &l.source {
                if let Err(e) = Url::parse(source) {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid source URL: {}. Reason: {}",
                        source, e
                    )));
                }
            }
        }
        Ok(())
    }
}

pub fn validate_mod_logo(file: &mut ZipFile, return_bytes: bool) -> Result<Vec<u8>, ApiError> {
    let mut logo: Vec<u8> = vec![];
    if let Err(e) = file.read_to_end(&mut logo) {
        log::error!("{}", e);
        return Err(ApiError::BadRequest("Couldn't read logo.png".to_string()));
    }

    let mut reader = BufReader::new(Cursor::new(logo));

    let decoder = match PngDecoder::new(&mut reader) {
        Ok(d) => d,
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::BadRequest("Invalid logo.png".to_string()));
        }
    };
    let mut img = match DynamicImage::from_decoder(decoder) {
        Ok(i) => i,
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::BadRequest("Invalid logo.png".to_string()));
        }
    };

    let dimensions = img.dimensions();

    if dimensions.0 != dimensions.1 {
        return Err(ApiError::BadRequest(format!(
            "Mod logo must have 1:1 aspect ratio. Current size is {}x{}",
            dimensions.0, dimensions.1
        )));
    }

    if (dimensions.0 > 336) || (dimensions.1 > 336) {
        img = img.resize(336, 336, image::imageops::FilterType::Lanczos3);
    }

    if !return_bytes {
        return Ok(vec![]);
    }

    let mut cursor: Cursor<Vec<u8>> = Cursor::new(vec![]);

    let encoder = PngEncoder::new_with_quality(
        &mut cursor,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::NoFilter,
    );

    let (width, height) = img.dimensions();

    if let Err(e) = encoder.write_image(img.as_bytes(), width, height, img.color().into()) {
        log::error!("{}", e);
        return Err(ApiError::BadRequest("Invalid logo.png".to_string()));
    }
    cursor.seek(std::io::SeekFrom::Start(0)).unwrap();

    let mut bytes: Vec<u8> = vec![];
    cursor.read_to_end(&mut bytes).unwrap();

    Ok(bytes)
}

fn parse_zip_entry_to_str(file: &mut ZipFile) -> Result<String, String> {
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

fn check_mac_binary(file: &mut ZipFile) -> Result<(bool, bool), ApiError> {
    // 12 bytes is all we need
    let mut bytes: Vec<u8> = vec![0; 12];
    file.read_exact(&mut bytes).map_err(|e| {
        log::error!("Failed to read MacOS binary: {}", e);
        ApiError::BadRequest("Invalid MacOS binary".into())
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
                return Err(ApiError::BadRequest("Invalid MacOS binary".to_string()));
            }
        } else if num_arches == 0x2 {
            return Ok((true, true));
        } else {
            return Err(ApiError::BadRequest("Invalid MacOS binary".to_string()));
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
    Err(ApiError::BadRequest("Invalid MacOS binary".to_string()))
}
