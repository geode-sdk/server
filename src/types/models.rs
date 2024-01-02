use serde::Serialize;

#[derive(Serialize)]
pub struct Mod {
    pub id: Option<String>,
    pub name: Option<String>,
    pub developer: Option<String>,
    pub download_url: Option<String>,
}