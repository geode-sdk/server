
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use crate::types::api::ApiError;

use super::{developer::Developer, mod_entity::Mod};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Stats {
    pub total_geode_downloads: i64,
    pub total_mod_downloads: i64,
    pub total_registered_developers: i64,
}

impl Stats {
    pub async fn get_cached(pool: &mut PgConnection) -> Result<Stats, ApiError> {
        Ok(Stats {
            total_mod_downloads: Mod::get_total_count(&mut *pool).await?,
            total_registered_developers: Developer::get_total_count(&mut *pool).await?,
            total_geode_downloads: 0,
        })
    }
}
