use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct Mod {
    pub id: String,
    pub repository: String,
    pub latest_version: String,
    pub validated: bool
}

#[derive(Serialize, Debug)]
pub struct ModVersion {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub download_link: String,
    pub hash: String,
    pub geode_version: String,
    pub windows: bool,
    pub android32: bool,
    pub android64: bool,
    pub mac: bool,
    pub ios: bool,
    pub mod_id: String
}