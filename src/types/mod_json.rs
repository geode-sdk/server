use std::{fs::File, path::Path};

use serde::Deserialize;
use zip::read::ZipFile;
use std::io::BufReader;

use super::api::ApiError;

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
    pub hash: String
}

impl ModJson {
    pub fn from_zip(file_path: &String, download_url: &str) -> Result<ModJson, ApiError> {
        let file = File::open(&file_path).or(Err(ApiError::FilesystemError))?;
        let path = Path::new(file_path);
        let hash = sha256::try_digest(path).or(Err(ApiError::FilesystemError))?;
        let reader = BufReader::new(file);
        let archive_res = zip::ZipArchive::new(reader);
        if archive_res.is_err() {
            return Err(ApiError::FilesystemError)
        }
        let mut archive = archive_res.unwrap();
        let json_file = archive.by_name("mod.json").or(Err(ApiError::BadRequest(String::from("mod.json not found"))))?;
        let mut json = serde_json::from_reader::<ZipFile, ModJson>(json_file)
            .or(Err(ApiError::BadRequest(String::from("Invalid mod.json"))))?;
        json.hash = hash;
        json.download_url = download_url.to_string();

        for i in 0..archive.len() {
            if let Some(file) = archive.by_index(i).ok() {
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
                if file.name().ends_with(".v7.so") {
                    json.android32 = true;
                    continue;
                }
                if file.name().ends_with(".v8.so") {
                    json.android64 = true;
                    continue;
                }
            }
        }
        return Ok(json);
    }
}