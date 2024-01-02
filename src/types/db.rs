use serde::Serialize;

#[derive(Serialize)]
pub struct Mod {
    id: String,
    name: String,
    download_url: String,
}