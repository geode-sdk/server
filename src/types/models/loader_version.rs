use std::collections::HashMap;

use crate::{
    database::DatabaseError,
    types::{
        models::mod_gd_version::{DetailedGDVersion, GDVersionEnum, VerPlatform},
        serde::chrono_dt_secs,
    },
};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use sqlx::{
    PgConnection, Postgres, QueryBuilder,
    types::chrono::{DateTime, Utc},
};

#[derive(Debug)]
pub struct LoaderVersionCreate {
    pub tag: String,
    pub prerelease: bool,
    pub commit_hash: String,
    pub mac: Option<GDVersionEnum>,
    pub win: Option<GDVersionEnum>,
    pub android: Option<GDVersionEnum>,
    pub ios: Option<GDVersionEnum>,
}

#[derive(Serialize, Deserialize, Default, Debug, ToSchema)]
pub struct LoaderDownload {
    pub url: String,
    #[serde(serialize_with = "serialize_nonempty_str")]
    pub hash: String,
}

#[derive(Serialize, Deserialize, Default, Debug, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct LoaderDownloads {
    pub win: LoaderDownload,
    pub mac: LoaderDownload,
    pub android32: LoaderDownload,
    pub android64: LoaderDownload,
    pub ios: LoaderDownload,
    pub resources: LoaderDownload,
    pub win_installer: LoaderDownload,
    pub mac_installer: LoaderDownload,
    pub linux_installer: LoaderDownload,
}

#[derive(Serialize, Debug, ToSchema)]
pub struct LoaderVersion {
    pub version: String,
    pub tag: String,
    pub gd: DetailedGDVersion,
    pub prerelease: bool,
    pub commit_hash: String,
    #[serde(with = "chrono_dt_secs")]
    pub created_at: DateTime<Utc>,
    pub downloads: LoaderDownloads,
}

#[derive(sqlx::FromRow, Debug)]
pub struct LoaderVersionGetOne {
    pub tag: String,
    pub prerelease: bool,
    pub commit_hash: String,
    pub created_at: DateTime<Utc>,
    pub mac: Option<GDVersionEnum>,
    pub win: Option<GDVersionEnum>,
    pub android: Option<GDVersionEnum>,
    pub ios: Option<GDVersionEnum>,

    pub resources_url: Option<String>,
    pub resources_hash: Option<String>,
}

#[derive(sqlx::FromRow, Debug)]
pub struct GeodeVersionDownload {
    pub tag: String,
    pub platform: String,
    pub url: String,
    pub hash: String,
}

pub struct GetVersionsQuery {
    pub gd: Option<GDVersionEnum>,
    pub platform: Option<VerPlatform>,
    pub prerelease: bool,
}

fn github_url(tag: &str, platform: &str) -> String {
    format!(
        "https://github.com/geode-sdk/geode/releases/download/v{tag}/geode-v{tag}-{platform}.zip"
    )
}

fn github_installer_url(tag: &str, platform_ext: &str) -> String {
    format!(
        "https://github.com/geode-sdk/geode/releases/download/v{tag}/geode-installer-v{tag}-{platform_ext}"
    )
}

fn github_resources_url(tag: &str) -> String {
    format!("https://github.com/geode-sdk/geode/releases/download/v{tag}/resources.zip")
}

impl LoaderDownload {
    pub fn new_github(tag: &str, platform: &str) -> Self {
        // this should only be called for versions that weren't migrated to S3 yet,
        // temporarily serve the GitHub URLs and tell the client to not verify hashes
        LoaderDownload {
            url: github_url(tag, platform),
            hash: String::new(),
        }
    }

    pub fn new_github_installer(tag: &str, platform_ext: &str) -> Self {
        LoaderDownload {
            url: github_installer_url(tag, platform_ext),
            hash: String::new(),
        }
    }

    pub fn new_github_resources(tag: &str) -> Self {
        LoaderDownload {
            url: github_resources_url(tag),
            hash: String::new(),
        }
    }
}

fn build_downloads(
    version: &LoaderVersionGetOne,
    managed: Vec<GeodeVersionDownload>,
) -> LoaderDownloads {
    let mut out = LoaderDownloads {
        win: LoaderDownload::new_github(&version.tag, "windows"),
        mac: LoaderDownload::new_github(&version.tag, "macos"),
        android32: LoaderDownload::new_github(&version.tag, "android32"),
        android64: LoaderDownload::new_github(&version.tag, "android64"),
        ios: LoaderDownload::new_github(&version.tag, "ios"),
        win_installer: LoaderDownload::new_github_installer(&version.tag, "win.exe"),
        mac_installer: LoaderDownload::new_github_installer(&version.tag, "mac.pkg"),
        linux_installer: LoaderDownload::new_github_installer(&version.tag, "linux.sh"),
        resources: LoaderDownload::new_github_resources(&version.tag),
    };

    for d in managed {
        let download = LoaderDownload {
            url: d.url,
            hash: d.hash,
        };

        match d.platform.as_str() {
            "win" => out.win = download,
            "mac" => out.mac = download,
            "android32" => out.android32 = download,
            "android64" => out.android64 = download,
            "ios" => out.ios = download,
            "win-installer" => out.win_installer = download,
            "mac-installer" => out.mac_installer = download,
            "linux-installer" => out.linux_installer = download,
            _ => {}
        }
    }

    if let Some(url) = version.resources_url.clone()
        && let Some(hash) = version.resources_hash.clone()
    {
        out.resources = LoaderDownload { url, hash };
    }

    out
}

impl LoaderVersionGetOne {
    pub fn into_loader_version(
        self,
        managed_downloads: Vec<GeodeVersionDownload>,
    ) -> LoaderVersion {
        let downloads = build_downloads(&self, managed_downloads);

        LoaderVersion {
            tag: format!("v{}", self.tag),
            version: self.tag,
            prerelease: self.prerelease,
            created_at: self.created_at,
            commit_hash: self.commit_hash,
            downloads,
            gd: DetailedGDVersion {
                win: self.win,
                mac: self.mac,
                mac_arm: self.mac,
                mac_intel: self.mac,
                android: self.android,
                android32: self.android,
                android64: self.android,
                ios: self.ios,
            },
        }
    }
}

impl LoaderVersion {
    pub async fn get_downloads_for_tag(
        tag: &str,
        pool: &mut PgConnection,
    ) -> Result<Vec<GeodeVersionDownload>, DatabaseError> {
        Ok(sqlx::query_as::<_, GeodeVersionDownload>(
            "SELECT tag, platform, url, hash FROM geode_version_download WHERE tag = $1",
        )
        .bind(tag)
        .fetch_all(&mut *pool)
        .await?)
    }

    pub async fn get_downloads_for_tags(
        tags: &[String],
        pool: &mut PgConnection,
    ) -> Result<HashMap<String, Vec<GeodeVersionDownload>>, DatabaseError> {
        if tags.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query_as::<_, GeodeVersionDownload>(
            "SELECT tag, platform, url, hash FROM geode_version_download WHERE tag = ANY($1)",
        )
        .bind(tags)
        .fetch_all(&mut *pool)
        .await?;

        let mut map = HashMap::<_, Vec<_>>::with_capacity(rows.len());
        for row in rows {
            map.entry(row.tag.clone()).or_default().push(row);
        }
        Ok(map)
    }

    #[tracing::instrument(skip_all, fields(gd = ?gd, platform = ?platform, accept_prereleases = %accept_prereleases))]
    pub async fn get_latest(
        gd: Option<GDVersionEnum>,
        platform: Option<VerPlatform>,
        accept_prereleases: bool,
        pool: &mut PgConnection,
    ) -> Result<Option<LoaderVersion>, DatabaseError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT
                mac, win, android, ios, tag, commit_hash, created_at, prerelease, resources_url, resources_hash
                FROM geode_versions
            "#,
        );

        match (platform, gd) {
            (Some(p), Some(g)) => {
                match p {
                    VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => {
                        query_builder.push(" WHERE android=")
                    }
                    VerPlatform::Mac | VerPlatform::MacIntel | VerPlatform::MacArm => {
                        query_builder.push(" WHERE mac=")
                    }
                    VerPlatform::Ios => query_builder.push(" WHERE ios="),
                    VerPlatform::Win => query_builder.push(" WHERE win="),
                    // _ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
                };

                query_builder.push_bind(g);
            }
            (Some(_), None) => {
                // this option will be handled later by ordering tricks
                query_builder.push(" WHERE 1=1");
            }
            (None, Some(g)) => {
                query_builder.push(" WHERE (android=");
                query_builder.push_bind(g);
                query_builder.push(" or mac=");
                query_builder.push_bind(g);
                query_builder.push(" or win=");
                query_builder.push_bind(g);
                query_builder.push(" or ios=");
                query_builder.push_bind(g);
                query_builder.push(')');
            }
            (None, None) => {
                // if gd version isn't specifed, select whatever versions have the latest gd version
                query_builder.push(
                    r#" WHERE
                    (android=enum_last(NULL::gd_version) OR
                    win=enum_last(NULL::gd_version) OR
                    mac=enum_last(NULL::gd_version) OR
                    ios=enum_last(NULL::gd_version))
                    "#,
                );
            }
        }

        if !accept_prereleases {
            query_builder.push(" AND prerelease=FALSE ");
        }

        // prioritize releases that can be downloaded from a CDN
        query_builder.push(" ORDER BY resources_url IS NOT NULL DESC, ");

        if gd.is_none()
            && let Some(p) = platform
        {
            // if there's a platform but no gd, order by the latest gd for that platform
            match p {
                VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => {
                    query_builder.push(" android")
                }
                VerPlatform::Mac | VerPlatform::MacIntel | VerPlatform::MacArm => {
                    query_builder.push(" mac")
                }
                VerPlatform::Win => query_builder.push(" win"),
                VerPlatform::Ios => query_builder.push(" ios"),
                // _ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
            };
            query_builder.push(" DESC, ");
        }

        query_builder.push(" created_at DESC LIMIT 1;");

        let Some(row) = query_builder
            .build_query_as::<LoaderVersionGetOne>()
            .fetch_optional(&mut *pool)
            .await
            .inspect_err(|e| tracing::error!("{:?}", e))?
        else {
            return Ok(None);
        };

        let downloads = LoaderVersion::get_downloads_for_tag(&row.tag, &mut *pool).await?;
        Ok(Some(row.into_loader_version(downloads)))
    }

    #[tracing::instrument(skip_all, fields(tag = %tag))]
    pub async fn get_one(
        tag: &str,
        pool: &mut PgConnection,
    ) -> Result<Option<LoaderVersion>, DatabaseError> {
        let Some(row) = sqlx::query_as!(
            LoaderVersionGetOne,
            r#"SELECT
				        mac as "mac: _", win as "win: _", android as "android: _", ios as "ios: _",
				        tag, created_at, commit_hash, prerelease, resources_url, resources_hash
			      FROM geode_versions
				    WHERE tag = $1"#,
            tag
        )
        .fetch_optional(&mut *pool)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?
        else {
            return Ok(None);
        };

        let downloads = LoaderVersion::get_downloads_for_tag(&row.tag, &mut *pool).await?;
        Ok(Some(row.into_loader_version(downloads)))
    }

    #[tracing::instrument(skip_all, fields(tag = %version.tag))]
    pub async fn create_version(
        version: LoaderVersionCreate,
        pool: &mut PgConnection,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            r#"INSERT INTO geode_versions
                (tag, prerelease, mac, win, android, ios, commit_hash)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7)"#,
            version.tag,
            version.prerelease,
            version.mac as _,
            version.win as _,
            version.android as _,
            version.ios as _,
            version.commit_hash
        )
        .execute(&mut *pool)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))
        .map(|_| ())
        .map_err(|e| e.into())
    }

    #[tracing::instrument(skip_all, fields(page = %page, per_page = %per_page))]
    pub async fn get_many(
        query: GetVersionsQuery,
        per_page: i64,
        page: i64,
        pool: &mut PgConnection,
    ) -> Result<Vec<LoaderVersion>, DatabaseError> {
        let limit = per_page;
        let offset = (page - 1) * per_page;

        let mut query_builder = QueryBuilder::new(
            r#"
            SELECT
                mac, win, android, ios, tag, created_at, commit_hash, prerelease, resources_url, resources_hash
            FROM geode_versions
            "#,
        );

        match (query.platform, query.gd) {
            (Some(p), Some(g)) => {
                match p {
                    VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => {
                        query_builder.push(" WHERE android=")
                    }
                    VerPlatform::Mac | VerPlatform::MacIntel | VerPlatform::MacArm => {
                        query_builder.push(" WHERE mac=")
                    }
                    VerPlatform::Ios => query_builder.push(" WHERE ios="),
                    VerPlatform::Win => query_builder.push(" WHERE win="),
                    // _ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
                };

                query_builder.push_bind(g);
            }
            (Some(p), None) => {
                match p {
                    VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => {
                        query_builder.push(" WHERE android IS NOT NULL")
                    }
                    VerPlatform::Mac | VerPlatform::MacIntel | VerPlatform::MacArm => {
                        query_builder.push(" WHERE mac IS NOT NULL")
                    }
                    VerPlatform::Ios => query_builder.push(" WHERE ios IS NOT NULL"),
                    VerPlatform::Win => query_builder.push(" WHERE win IS NOT NULL"),
                    // _ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
                };
            }
            (None, Some(g)) => {
                query_builder.push(" WHERE (android=");
                query_builder.push_bind(g);
                query_builder.push(" or mac=");
                query_builder.push_bind(g);
                query_builder.push(" or win=");
                query_builder.push_bind(g);
                query_builder.push(" or ios=");
                query_builder.push_bind(g);
                query_builder.push(')');
            }
            _ => {
                query_builder.push(" WHERE 1=1");
            }
        }

        if !query.prerelease {
            query_builder.push(" AND prerelease=FALSE ");
        }

        query_builder.push(" ORDER BY created_at DESC ");

        query_builder.push(" LIMIT ");
        query_builder.push_bind(limit);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset);

        let rows = query_builder
            .build_query_as::<LoaderVersionGetOne>()
            .fetch_all(&mut *pool)
            .await
            .inspect_err(|e| tracing::error!("{:?}", e))
            .map_err(DatabaseError::from)?;

        let tags: Vec<String> = rows.iter().map(|r| r.tag.clone()).collect();
        let mut downloads_map = LoaderVersion::get_downloads_for_tags(&tags, &mut *pool).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let downloads = downloads_map.remove(&row.tag).unwrap_or_default();
                row.into_loader_version(downloads)
            })
            .collect())
    }
}

fn serialize_nonempty_str<S>(s: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if s.is_empty() {
        serializer.serialize_none()
    } else {
        serializer.serialize_str(s)
    }
}
