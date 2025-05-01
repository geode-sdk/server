use serde::Serialize;
use sqlx::PgConnection;

use crate::types::api::ApiError;

#[derive(Serialize, Debug, Clone)]
pub struct ModLinks {
    #[serde(skip_serializing)]
    pub mod_id: String,
    pub community: Option<String>,
    pub homepage: Option<String>,
    pub source: Option<String>,
}

impl ModLinks {
    pub async fn fetch(
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<Option<ModLinks>, ApiError> {
        match sqlx::query_as!(
            ModLinks,
            "SELECT
                mod_id, community, homepage, source
            FROM mod_links
            WHERE mod_id = $1",
            mod_id
        )
        .fetch_optional(pool)
        .await
        {
            Ok(r) => Ok(r),
            Err(e) => {
                log::error!("Failed to fetch mod links for mod {}. Error: {}", mod_id, e);
                Err(ApiError::DbError)
            }
        }
    }

    pub async fn fetch_for_mods(
        mod_ids: &Vec<String>,
        pool: &mut PgConnection,
    ) -> Result<Vec<ModLinks>, ApiError> {
        if mod_ids.is_empty() {
            return Ok(vec![]);
        }

        match sqlx::query_as!(
            ModLinks,
            "SELECT
                mod_id, community, homepage, source
            FROM mod_links
            WHERE mod_id = ANY($1)",
            mod_ids
        )
        .fetch_all(pool)
        .await
        {
            Err(e) => {
                log::error!("Failed to fetch mod links for multiple mods. Error: {}", e);
                Err(ApiError::DbError)
            }
            Ok(r) => Ok(r),
        }
    }
}
