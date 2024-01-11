use std::fs::File;

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
    #[serde(default="bool::default")]
    pub windows: bool,
    #[serde(default="bool::default")]
    pub ios: bool,
    #[serde(default="bool::default")]
    pub android32: bool,
    #[serde(default="bool::default")]
    pub android64: bool,
    #[serde(default="bool::default")]
    pub mac: bool
}

impl ModJson {
    pub fn from_zip(file_path: &String) -> Result<ModJson, ApiError> {
        let file = File::open(&file_path).or(Err(ApiError::FilesystemError))?;
        let reader = BufReader::new(file);
        let archive_res = zip::ZipArchive::new(reader);
        if archive_res.is_err() {
            return Err(ApiError::FilesystemError)
        }
        let mut archive = archive_res.unwrap();
        let json_file = archive.by_name("mod.json").or(Err(ApiError::BadRequest(String::from("mod.json not found"))))?;
        let mut json = serde_json::from_reader::<ZipFile, ModJson>(json_file)
            .or(Err(ApiError::BadRequest(String::from("Invalid mod.json"))))?;

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