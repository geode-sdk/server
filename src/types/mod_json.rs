use std::io::{Cursor, Read, Seek};

use actix_web::web::Bytes;
use image::{
    codecs::png::{PngDecoder, PngEncoder},
    DynamicImage, GenericImageView, ImageEncoder,
};
use regex::Regex;
use semver::Version;
use serde::Deserialize;
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
    pub api: Option<serde_json::Value>,
    pub gd: ModJsonGDVersionType,
    #[serde(skip_deserializing)]
    pub logo: Vec<u8>,
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
        json.version = json.version.trim_start_matches('v').to_string();
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
                i.version = i.version.trim_start_matches('v').to_string();
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
                i.version = i.version.trim_start_matches('v').to_string();
            }
        }

        for i in 0..archive.len() {
            if let Ok(mut file) = archive.by_index(i) {
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

                if file.name() == "logo.png" {
                    let bytes = validate_mod_logo(&mut file)?;
                    json.logo = bytes;
                    continue;
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

        if deps.is_empty() {
            return Ok(vec![]);
        }

        let mut ret: Vec<DependencyCreate> = vec![];

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
            let (dependency_ver, compare) = match split_version_and_compare(i.version.as_str()) {
                Err(_) => {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid semver {}",
                        i.version
                    )))
                }
                Ok((ver, compare)) => (ver, compare),
            };
            ret.push(DependencyCreate {
                dependency_id: i.id.clone(),
                version: dependency_ver.to_string(),
                compare,
                importance: i.importance,
            });
        }

        Ok(ret)
    }

    pub fn prepare_incompatibilities_for_create(
        &self,
    ) -> Result<Vec<IncompatibilityCreate>, ApiError> {
        let incompat = match self.incompatibilities.as_ref() {
            None => return Ok(vec![]),
            Some(d) => d,
        };

        if incompat.is_empty() {
            return Ok(vec![]);
        }
        let mut ret: Vec<IncompatibilityCreate> = vec![];

        for i in incompat {
            if i.version == "*" {
                ret.push(IncompatibilityCreate {
                    incompatibility_id: i.id.clone(),
                    version: "*".to_string(),
                    compare: ModVersionCompare::MoreEq,
                    importance: i.importance,
                });
                continue;
            }
            let (ver, compare) = match split_version_and_compare(i.version.as_str()) {
                Err(_) => {
                    return Err(ApiError::BadRequest(format!(
                        "Invalid semver {}",
                        i.version
                    )))
                }
                Ok((ver, compare)) => (ver, compare),
            };
            ret.push(IncompatibilityCreate {
                incompatibility_id: i.id.clone(),
                version: ver.to_string(),
                compare,
                importance: i.importance,
            });
        }

        Ok(ret)
    }

    pub fn validate(&self) -> Result<(), ApiError> {
        let id_regex = Regex::new(r#"^[a-z0-9_\-]+\.[a-z0-9_\-]+$"#).unwrap();
        if !id_regex.is_match(&self.id) {
            return Err(ApiError::BadRequest(format!(
                "Invalid mod id {} (lowercase and numbers only, needs to look like 'dev.mod')",
                self.id
            )));
        }

        if self.id.len() > 64 {
            return Err(ApiError::BadRequest(
                "Mod id too long (max 64 characters)".to_string(),
            ));
        }
        Ok(())
    }
}

fn validate_mod_logo(file: &mut ZipFile) -> Result<Vec<u8>, ApiError> {
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
    let img = match DynamicImage::from_decoder(decoder) {
        Ok(i) => i,
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::BadRequest("Invalid logo.png".to_string()));
        }
    };

    let dimensions = img.dimensions();
    if (dimensions.0 > 336) || (dimensions.1 > 336) {
        img.resize(336, 336, image::imageops::FilterType::Lanczos3);
    }

    let mut cursor: Cursor<Vec<u8>> = Cursor::new(vec![]);

    let encoder = PngEncoder::new_with_quality(
        &mut cursor,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::NoFilter,
    );

    let (width, height) = img.dimensions();

    encoder
        .write_image(img.as_bytes(), width, height, image::ColorType::Rgba8)
        .or(Err(ApiError::FilesystemError))?;
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

fn validate_dependency_version_str(ver: &str) -> bool {
    if ver == "*" {
        return true;
    }
    let mut copy = ver.to_string();
    if ver.starts_with("<=") {
        copy = copy.trim_start_matches("<=").to_string();
    } else if ver.starts_with(">=") {
        copy = copy.trim_start_matches(">=").to_string();
    } else if ver.starts_with('=') {
        copy = copy.trim_start_matches('=').to_string();
    } else if ver.starts_with('<') {
        copy = copy.trim_start_matches('<').to_string();
    } else if ver.starts_with('>') {
        copy = copy.trim_start_matches('>').to_string();
    }
    copy = copy.trim_start_matches('v').to_string();

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
