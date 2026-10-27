use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct PaginatedData<T> {
    pub data: Vec<T>,
    pub count: i64,
}

pub fn create_download_link(app_url: &str, mod_id: &str, version: &str) -> String {
    format!(
        "{}/v1/mods/{}/versions/{}/download",
        app_url, mod_id, version
    )
}
