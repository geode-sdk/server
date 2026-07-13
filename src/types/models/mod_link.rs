use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::database::DatabaseError;

#[derive(Serialize, Debug, Clone, ToSchema)]
pub struct ModLinks {
    #[serde(skip_serializing)]
    pub mod_id: String,
    pub community: Option<String>,
    pub homepage: Option<String>,
    pub source: Option<String>,
}

impl ModLinks {
    #[tracing::instrument(skip_all, fields(mod_id = %mod_id))]
    pub async fn fetch(
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<Option<ModLinks>, DatabaseError> {
        sqlx::query_as!(
            ModLinks,
            "SELECT
                mod_id, community, homepage, source
            FROM mod_links
            WHERE mod_id = $1",
            mod_id
        )
        .fetch_optional(pool)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))
        .map_err(|e| e.into())
    }

    #[tracing::instrument(skip_all, fields(mod_ids = ?mod_ids))]
    pub async fn fetch_for_mods(
        mod_ids: &Vec<String>,
        pool: &mut PgConnection,
    ) -> Result<Vec<ModLinks>, DatabaseError> {
        if mod_ids.is_empty() {
            return Ok(vec![]);
        }

        sqlx::query_as!(
            ModLinks,
            "SELECT
                mod_id, community, homepage, source
            FROM mod_links
            WHERE mod_id = ANY($1)",
            mod_ids
        )
        .fetch_all(pool)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))
        .map_err(|e| e.into())
    }
}
