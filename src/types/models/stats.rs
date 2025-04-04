use crate::database::repository::developers;
use crate::types::api::ApiError;
use chrono::Utc;
use reqwest::{header::HeaderValue, Client};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use super::mod_entity::Mod;

#[derive(Deserialize)]
struct GithubReleaseAsset {
    // Github says in its API specs it's an integer, so theoretically it might
    // return a download count of -1 or something
    download_count: i64,
}

#[derive(Deserialize)]
struct GithubReleaseWithAssets {
    tag_name: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Stats {
    pub total_geode_downloads: i64,
    pub total_mod_count: i64,
    pub total_mod_downloads: i64,
    pub total_registered_developers: i64,
}

impl Stats {
    pub async fn get_cached(pool: &mut PgConnection) -> Result<Stats, ApiError> {
        let mod_stats = Mod::get_stats(&mut *pool).await?;
        Ok(Stats {
            total_mod_count: mod_stats.total_count,
            total_mod_downloads: mod_stats.total_downloads,
            total_registered_developers: developers::index_count("", &mut *pool).await?,
            total_geode_downloads: Self::get_latest_github_release_download_count(&mut *pool)
                .await?,
        })
    }

    async fn get_latest_github_release_download_count(
        pool: &mut PgConnection,
    ) -> Result<i64, ApiError> {
        // If release stats were fetched less than a day ago, just use cached stats
        if let Ok((cache_time, total_download_count)) = sqlx::query!(
            "
            SELECT s.checked_at, s.total_download_count
            FROM github_loader_release_stats s
            ORDER BY s.checked_at DESC
        "
        )
        .fetch_one(&mut *pool)
        .await
        .map(|d| (d.checked_at, d.total_download_count))
        {
            if Utc::now().signed_duration_since(cache_time).num_days() < 1 {
                return Ok(total_download_count);
            }
        }

        // Fetch latest stats
        let new = Self::fetch_github_release_stats().await?;
        sqlx::query!(
            "
            INSERT INTO github_loader_release_stats (total_download_count, latest_loader_version)
            VALUES ($1, $2)
        ",
            new.0,
            new.1
        )
        .execute(&mut *pool)
        .await
        .map_err(|e| {
            log::error!("{}", e);
            ApiError::DbError
        })?;
        Ok(new.0)
    }
    async fn fetch_github_release_stats() -> Result<(i64, String), ApiError> {
        let client = Client::new();
        let resp = client
            .get("https://api.github.com/repos/geode-sdk/geode/releases")
            .header("Accept", HeaderValue::from_str("application/json").unwrap())
            .header("User-Agent", "geode_index")
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| {
                log::info!("{}", e);
                ApiError::InternalError
            })?;
        if !resp.status().is_success() {
            return Err(ApiError::InternalError);
        }
        let releases: Vec<GithubReleaseWithAssets> = resp.json().await.map_err(|e| {
            log::info!("{}", e);
            ApiError::InternalError
        })?;
        let latest_release_tag = releases
            .iter()
            .find(|r| r.tag_name != "nightly")
            .ok_or(ApiError::InternalError)?
            .tag_name
            .clone();
        Ok((
            releases
                .into_iter()
                .map(|r| r.assets.into_iter().map(|a| a.download_count).sum::<i64>())
                .sum(),
            latest_release_tag,
        ))
    }
}
