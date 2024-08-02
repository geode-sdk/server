use serde::Serialize;
use sqlx::PgConnection;

use crate::types::api::ApiError;

#[derive(Serialize, Debug, Clone)]
pub struct ModLinks {
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

    pub async fn upsert_for_mod(
        mod_id: &str,
        community: Option<String>,
        homepage: Option<String>,
        source: Option<String>,
        pool: &mut PgConnection,
    ) -> Result<Option<ModLinks>, ApiError> {
        if ModLinks::exists(mod_id, pool).await? {
            return ModLinks::update_for_mod(mod_id, community, homepage, source, pool).await;
        }

        match sqlx::query!(
            "INSERT INTO mod_links 
                (mod_id, community, homepage, source) 
            VALUES 
                ($1, $2, $3, $4)",
            mod_id,
            community,
            homepage,
            source
        )
        .execute(pool)
        .await
        {
            Ok(_) => Ok(Some(ModLinks {
                mod_id: mod_id.to_string(),
                community,
                homepage,
                source,
            })),
            Err(e) => {
                log::error!("Failed to create mod link for {}. Error: {}", mod_id, e);
                Err(ApiError::DbError)
            }
        }
    }

    pub async fn exists(mod_id: &str, pool: &mut PgConnection) -> Result<bool, ApiError> {
        match sqlx::query!(
            "SELECT mod_id
            FROM mod_links
            WHERE mod_id = $1",
            mod_id
        )
        .fetch_optional(pool)
        .await
        {
            Ok(r) => Ok(r.is_some()),
            Err(e) => {
                log::error!(
                    "Failed to check if mod links exist for {}. Error: {}",
                    mod_id,
                    e
                );
                Err(ApiError::DbError)
            }
        }
    }

    async fn update_for_mod(
        mod_id: &str,
        community: Option<String>,
        homepage: Option<String>,
        source: Option<String>,
        pool: &mut PgConnection,
    ) -> Result<Option<ModLinks>, ApiError> {
        if community.is_none() && homepage.is_none() && source.is_none() {
            ModLinks::delete_for_mod(mod_id, pool).await?;
            return Ok(None);
        }

        match sqlx::query!(
            "UPDATE mod_links
            SET community = $1,
                homepage = $2,
                source = $3
            WHERE mod_id = $4",
            community,
            homepage,
            source,
            mod_id
        )
        .execute(pool)
        .await
        {
            Err(e) => {
                log::error!("Failed to update mod links for {}. Error: {}", mod_id, e);
                Err(ApiError::DbError)
            }
            Ok(r) => {
                if r.rows_affected() == 0 {
                    log::error!(
                        "Failed to update mod links for {}. No rows affected.",
                        mod_id
                    );
                    Err(ApiError::DbError)
                } else {
                    Ok(Some(ModLinks {
                        mod_id: mod_id.to_string(),
                        community,
                        homepage,
                        source,
                    }))
                }
            }
        }
    }

    async fn delete_for_mod(mod_id: &str, pool: &mut PgConnection) -> Result<(), ApiError> {
        match sqlx::query!(
            "DELETE FROM mod_links
            WHERE mod_id = $1",
            mod_id
        )
        .execute(pool)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                log::error!("Failed to delete mod_links for {}. Error: {}", mod_id, e);
                Err(ApiError::DbError)
            }
        }
    }
}
