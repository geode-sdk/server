use super::{
    dependency::ResponseDependency,
    developer::ModDeveloper,
    download_count::DownloadCount,
    incompatibility::{Replacement, ResponseIncompatibility},
    mod_gd_version::{DetailedGDVersion, GDVersionEnum, ModGDVersion, VerPlatform},
    mod_link::ModLinks,
    tag::Tag,
};
use crate::{
    database::{DatabaseError, repository::developers},
    endpoints::ApiError,
};
use crate::{
    endpoints::{
        developers::{SimpleDevMod, SimpleDevModVersion},
        mods::{IndexQueryParams, IndexSortType},
    },
    types::{
        api::PaginatedData,
        models::{mod_version::ModVersion, mod_version_status::ModVersionStatusEnum},
        serde::chrono_dt_secs,
    },
};
use semver::Version;
use serde::Serialize;
use sqlx::{
    PgConnection,
    types::chrono::{DateTime, Utc},
};
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Serialize, Debug, Clone, sqlx::FromRow, ToSchema)]
pub struct Mod {
    pub id: String,
    pub repository: Option<String>,
    pub featured: bool,
    #[schema(value_type = i32)]
    pub download_count: DownloadCount,
    pub developers: Vec<ModDeveloper>,
    pub versions: Vec<ModVersion>,
    pub tags: Vec<String>,
    pub about: Option<String>,
    pub changelog: Option<String>,
    #[serde(with = "chrono_dt_secs")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono_dt_secs")]
    pub updated_at: DateTime<Utc>,
    pub links: Option<ModLinks>,
}

#[derive(Serialize, Debug, ToSchema)]
pub struct ModUpdate {
    pub id: String,
    pub version: String,
    #[serde(skip_serializing)]
    pub mod_version_id: i32,
    pub download_link: String,
    pub replacement: Option<Replacement>,
    pub dependencies: Vec<ResponseDependency>,
    pub incompatibilities: Vec<ResponseIncompatibility>,
}

#[derive(Debug, sqlx::FromRow)]
struct ModRecord {
    id: String,
    #[sqlx(default)]
    repository: Option<String>,
    download_count: i32,
    featured: bool,
    about: Option<String>,
    changelog: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ModRecordGetOne {
    id: String,
    repository: Option<String>,
    featured: bool,
    version_id: i32,
    mod_download_count: i32,
    name: String,
    description: Option<String>,
    version: String,
    download_link: String,
    managed_download_link: Option<String>,
    mod_version_download_count: i32,
    hash: String,
    geode: String,
    early_load: bool,
    requires_patching: bool,
    api: bool,
    mod_id: String,
    status: ModVersionStatusEnum,
    about: Option<String>,
    changelog: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    mod_version_created_at: Option<DateTime<Utc>>,
    mod_version_updated_at: Option<DateTime<Utc>>,
    info: Option<String>,
}

pub struct ModStats {
    pub total_count: i64,
    pub total_downloads: i64,
}

impl Mod {
    pub fn set_abbreviated_download_counts(&mut self, abbreviate: bool) {
        self.download_count.set_abbreviated(abbreviate);
        for version in &mut self.versions {
            version.set_abbreviated_download_count(abbreviate);
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_stats(pool: &mut PgConnection) -> Result<ModStats, DatabaseError> {
        let result = sqlx::query!(
            "
            SELECT COUNT(id) as id_count, SUM(download_count) as download_sum
            FROM (
                select m.id, m.download_count, row_number() over(partition by m.id) rn
                FROM mods m
                INNER JOIN mod_versions mv ON mv.mod_id = m.id
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                WHERE mvs.status = 'accepted'
            ) q
            WHERE q.rn = 1
        "
        )
        .fetch_optional(&mut *pool)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?;

        if let Some((Some(total_count), Some(total_downloads))) =
            result.map(|o| (o.id_count, o.download_sum))
        {
            Ok(ModStats {
                total_count,
                total_downloads,
            })
        } else {
            Ok(ModStats {
                total_count: 0,
                total_downloads: 0,
            })
        }
    }

    #[tracing::instrument(skip_all, fields(query = ?query.query, page = ?query.page, per_page = ?query.per_page))]
    pub async fn get_index(
        pool: &mut PgConnection,
        query: &IndexQueryParams,
    ) -> Result<PaginatedData<Mod>, ApiError> {
        let tags = match &query.tags {
            Some(t) => Some(Tag::parse_tags(t, pool).await?),
            None => None,
        };
        let page: i64 = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

        let limit = per_page;
        let offset = (page - 1) * per_page;
        let platforms = query
            .platforms
            .as_ref()
            .map(|p| VerPlatform::parse_query_string(p))
            .transpose()?;
        let status = query.status.unwrap_or(ModVersionStatusEnum::Accepted);
        // We only want to filter anything if jitless is true
        let requires_patching = query.jitless.filter(|&j| j).map(|_| false);

        let developer = match &query.developer {
            Some(d) => match developers::get_one_by_username(d, pool).await? {
                Some(d) => Some(d),
                None => {
                    return Ok(PaginatedData {
                        data: vec![],
                        count: 0,
                    });
                }
            },
            None => None,
        };

        let order = match query.sort {
            IndexSortType::Downloads => "q.download_count DESC",
            IndexSortType::RecentlyUpdated => "q.updated_at DESC",
            IndexSortType::RecentlyPublished => "q.created_at DESC",
            IndexSortType::Oldest => "q.created_at ASC",
            IndexSortType::Name => "q.name ASC",
            IndexSortType::NameReverse => "q.name DESC",
            IndexSortType::Random => "RANDOM()",
        };

        let geode = query
            .geode
            .as_ref()
            .map(|x| Version::parse(x))
            .transpose()
            .or(Err(ApiError::BadRequest("Invalid geode version".into())))?;

        let geode_major = geode
            .as_ref()
            .map(|x| i32::try_from(x.major).unwrap_or_default());
        let geode_minor = geode
            .as_ref()
            .map(|x| i32::try_from(x.minor).unwrap_or_default());
        let geode_patch = geode
            .as_ref()
            .map(|x| i32::try_from(x.patch).unwrap_or_default());
        let geode_meta = geode.as_ref().and_then(|x| {
            if x.pre.is_empty() {
                None
            } else {
                Some(x.pre.to_string())
            }
        });

        let gd = query.gd.map(|x| vec![x, GDVersionEnum::All]);

        let core_query = |builder: &mut sqlx::QueryBuilder<sqlx::Postgres>| {
            // clone these due to silly lifetime rules, the closure lives till the
            // end of the function and the only other solution is ugly scopes
            let gd = gd.clone();
            let platforms = platforms.clone();
            let tags = tags.clone();
            let search_str = query.query.clone();
            let meta = geode_meta.clone();

            builder.push(" FROM MODS m ");

            // joins: only join tables if they are necessary
            builder.push(" INNER JOIN mod_versions mv ON m.id = mv.mod_id ");
            builder.push(" INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id ");

            if gd.is_some() || platforms.is_some() {
                builder.push(" INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id ");
            }

            if tags.is_some() {
                builder.push(" LEFT JOIN mods_mod_tags mmt ON mmt.mod_id = m.id ");
            }

            if developer.is_some() {
                builder.push(" INNER JOIN mods_developers md ON md.mod_id = m.id ");
            }

            // filters
            builder.push(" WHERE true ");

            if let Some(t) = tags {
                builder
                    .push(" AND mmt.tag_id = ANY(")
                    .push_bind(t)
                    .push(") ");
            }

            if let Some(f) = query.featured {
                builder.push(" AND m.featured = ").push_bind(f);
            }

            if let Some(d) = &developer {
                builder.push(" AND md.developer_id = ").push_bind(d.id);
            }

            builder.push(" AND mvs.status = ").push_bind(status);

            if let Some(rp) = requires_patching {
                builder.push(" AND mv.requires_patching = ").push_bind(rp);
            }

            if let Some(major) = geode_major {
                builder.push(" AND mv.geode_major = ").push_bind(major);
            }

            if let Some(minor) = geode_minor {
                builder.push(" AND mv.geode_minor <= ").push_bind(minor);
            }

            if let Some(s) = search_str {
                builder
                    .push(" AND (mv.name ILIKE '%' || ")
                    .push_bind(s.clone())
                    .push(" || '%' OR m.id = ")
                    .push_bind(s)
                    .push(") ");
            }

            if let Some(g) = gd {
                builder.push(" AND mgv.gd = ANY(").push_bind(g).push(") ");
            }

            if let Some(p) = platforms {
                builder
                    .push(" AND mgv.platform = ANY(")
                    .push_bind(p)
                    .push(") ");
            }

            // the cursed version comparison

            match meta {
                Some(meta) if meta.to_lowercase().starts_with("alpha") => {
                    builder
                        .push(" AND ( ")
                        .push("   (mv.geode_minor = ")
                        .push_bind(geode_minor)
                        .push("    AND mv.geode_patch = ")
                        .push_bind(geode_patch)
                        .push("    AND mv.geode_meta = ")
                        .push_bind(meta)
                        .push("   ) ")
                        .push("   OR mv.geode_meta IS NULL ")
                        .push("   OR mv.geode_minor < ")
                        .push_bind(geode_minor);

                    builder.push(" ) ");
                }

                Some(meta) => {
                    builder
                        .push(
                            " AND (mv.geode_meta IS NULL OR (mv.geode_meta NOT ILIKE 'alpha%' AND ",
                        )
                        .push_bind(meta)
                        .push(" >= mv.geode_meta)) ");
                }

                None => {
                    builder
                        .push(" AND (mv.geode_meta IS NULL OR mv.geode_meta NOT ILIKE 'alpha%') ");

                    if let Some(minor) = geode_minor {
                        builder
                            .push(" AND (mv.geode_minor <= ")
                            .push_bind(minor)
                            .push(") ");
                    }
                }
            }
        };

        let mut records_builder = sqlx::QueryBuilder::new(
            "SELECT q.id, q.repository, q.about, q.changelog,
                q.download_count, q.featured, q.created_at, q.updated_at
            FROM (
                SELECT DISTINCT ON (m.id) m.id, mv.name, m.repository, m.about, m.changelog,
                    m.download_count, m.featured, m.created_at, m.updated_at ",
        );

        core_query(&mut records_builder);

        records_builder.push(" ORDER BY m.id, mv.id DESC) q ");
        records_builder.push(format!(" ORDER BY {} ", order));
        records_builder.push(" LIMIT ").push_bind(limit);
        records_builder.push(" OFFSET ").push_bind(offset);

        // tracing::info!("sql: {:?}", records_builder.sql());

        let records: Vec<ModRecord> = records_builder
            .build_query_as()
            .fetch_all(&mut *pool)
            .await
            .inspect_err(|e| tracing::error!("{:?}", e))?;

        let mut count_builder = sqlx::QueryBuilder::new("SELECT COUNT(DISTINCT m.id) ");

        core_query(&mut count_builder);

        let count: i64 = count_builder
            .build_query_scalar()
            .fetch_optional(&mut *pool)
            .await
            .inspect_err(|e| tracing::error!("{:?}", e))?
            .unwrap_or_default();

        if records.is_empty() {
            return Ok(PaginatedData {
                data: vec![],
                count,
            });
        }

        if status == ModVersionStatusEnum::Pending {
            return Mod::get_pending(records, count, pool).await;
        }

        let ids: Vec<String> = records.iter().map(|x| x.id.clone()).collect();
        let mut versions = ModVersion::get_latest_for_mods(
            pool,
            &ids,
            query.gd,
            platforms.as_deref(),
            geode.as_ref(),
            requires_patching,
        )
        .await?;
        let mut developers = developers::get_all_for_mods(&ids, pool).await?;
        let links = ModLinks::fetch_for_mods(&ids, pool).await?;
        let mod_version_ids: Vec<i32> = versions
            .values()
            .map(|mod_version| mod_version.id)
            .collect();

        let mut gd_versions = ModGDVersion::get_for_mod_versions(&mod_version_ids, pool).await?;
        let mut tags = Tag::get_tags_for_mods(&ids, pool).await?;

        let ret = records
            .into_iter()
            .filter_map(|x| {
                let mut version = versions.remove(&x.id)?;
                version.gd = gd_versions.remove(&version.id).unwrap_or_default();

                let devs = developers.remove(&x.id).unwrap_or_default();
                let tags = tags.remove(&x.id).unwrap_or_default();
                let links = links.iter().find(|link| link.mod_id == x.id).cloned();

                Some(Mod {
                    id: x.id,
                    repository: x.repository,
                    download_count: x.download_count.into(),
                    featured: x.featured,
                    versions: vec![version],
                    tags,
                    developers: devs,
                    created_at: x.created_at,
                    updated_at: x.updated_at,
                    about: None,
                    changelog: None,
                    links,
                })
            })
            .collect();
        Ok(PaginatedData { data: ret, count })
    }

    async fn get_pending(
        records: Vec<ModRecord>,
        total_count: i64,
        pool: &mut PgConnection,
    ) -> Result<PaginatedData<Mod>, ApiError> {
        let ids: Vec<_> = records.iter().map(|x| x.id.clone()).collect();
        let versions = ModVersion::get_pending_for_mods(&ids, pool).await?;
        let developers = developers::get_all_for_mods(&ids, pool).await?;
        let links = ModLinks::fetch_for_mods(&ids, pool).await?;
        let mut mod_version_ids: Vec<i32> = vec![];
        for (_, mod_version) in versions.iter() {
            mod_version_ids.append(&mut mod_version.iter().map(|x| x.id).collect());
        }

        let gd_versions = ModGDVersion::get_for_mod_versions(&mod_version_ids, pool).await?;
        let tags = Tag::get_tags_for_mods(&ids, pool).await?;

        let ret = records
            .into_iter()
            .map(|x| {
                let mut version = versions.get(&x.id).cloned().unwrap_or_default();
                let gd_ver = gd_versions.get(&version[0].id).cloned().unwrap_or_default();
                version[0].gd = gd_ver;

                let devs = developers.get(&x.id).cloned().unwrap_or_default();
                let tags = tags.get(&x.id).cloned().unwrap_or_default();
                let links = links.iter().find(|link| link.mod_id == x.id).cloned();

                Mod {
                    id: x.id.clone(),
                    repository: x.repository.clone(),
                    download_count: x.download_count.into(),
                    featured: x.featured,
                    versions: version,
                    tags,
                    developers: devs,
                    created_at: x.created_at,
                    updated_at: x.updated_at,
                    about: x.about,
                    changelog: x.changelog,
                    links,
                }
            })
            .collect::<Vec<Mod>>();

        Ok(PaginatedData {
            data: ret,
            count: total_count,
        })
    }

    #[tracing::instrument(skip_all, fields(developer_id = %id, status = ?status, only_owner = %only_owner))]
    pub async fn get_all_for_dev(
        id: i32,
        status: ModVersionStatusEnum,
        only_owner: bool,
        pool: &mut PgConnection,
    ) -> Result<Vec<SimpleDevMod>, DatabaseError> {
        struct Record {
            id: String,
            featured: bool,
            mod_download_count: i32,
            name: String,
            version: String,
            mod_version_download_count: i32,
            validated: bool,
            status: ModVersionStatusEnum,
            info: Option<String>,
        }

        let records = sqlx::query_as!(
            Record,
            r#"SELECT
                m.id, m.featured, m.download_count as mod_download_count,
                mv.name, mv.version, mv.download_count as mod_version_download_count,
                mvs.info, mvs.status as "status: _",
                exists(
                    select 1 from mod_version_statuses mvs_inner
                    where mvs_inner.mod_version_id = mv.id and mvs_inner.status = 'accepted'
                ) as "validated!: _"
            FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mods_developers md ON md.mod_id = m.id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE md.developer_id = $1
            AND mvs.status = $2
            AND ($3 = false OR md.is_owner = true)
            ORDER BY m.created_at DESC, mv.id DESC
            "#,
            id,
            status as ModVersionStatusEnum,
            only_owner
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?;

        if records.is_empty() {
            return Ok(vec![]);
        }

        let mut versions: HashMap<String, Vec<SimpleDevModVersion>> = HashMap::new();

        for record in &records {
            let version = SimpleDevModVersion {
                name: record.name.clone(),
                version: record.version.clone(),
                download_count: record.mod_version_download_count,
                validated: record.validated,
                info: record.info.clone(),
                status: record.status,
            };

            versions.entry(record.id.clone()).or_default().push(version);
        }
        let ids: Vec<String> = records.iter().map(|x| x.id.clone()).collect();
        let developers = developers::get_all_for_mods(&ids, pool).await?;

        let mut map: HashMap<String, SimpleDevMod> = HashMap::new();

        for i in records {
            let mod_entity = SimpleDevMod {
                id: i.id.clone(),
                featured: i.featured,
                download_count: i.mod_download_count,
                versions: versions.entry(i.id.clone()).or_default().clone(),
                developers: developers.get(&i.id).cloned().unwrap_or_default(),
            };
            if !map.contains_key(&i.id) {
                map.insert(i.id.clone(), mod_entity);
            }
        }

        let mods: Vec<SimpleDevMod> = map.into_iter().map(|x| x.1).collect();

        Ok(mods)
    }

    #[tracing::instrument(skip_all, fields(mod_id = %id, only_accepted = %only_accepted))]
    pub async fn get_one(
        id: &str,
        only_accepted: bool,
        pool: &mut PgConnection,
    ) -> Result<Option<Mod>, DatabaseError> {
        let records = sqlx::query_as!(
            ModRecordGetOne,
            r#"SELECT
                m.id, m.repository, m.about, m.changelog, m.featured, m.download_count as mod_download_count, m.created_at, m.updated_at,
                mv.id as version_id, mv.name, mv.description, mv.version, mv.download_link, mv.managed_download_link,
                mv.download_count as mod_version_download_count, mv.created_at as mod_version_created_at, mv.updated_at as mod_version_updated_at, mv.hash,
                format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as "geode!: _",
                mv.early_load, mv.requires_patching, mv.api, mv.mod_id, mvs.status as "status: _", mvs.info
            FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE m.id = $1
            AND ($2 = false OR mvs.status = 'accepted')
            ORDER BY mv.id DESC"#,
            id,
            only_accepted
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?;

        if records.is_empty() {
            return Ok(None);
        }

        let mut versions: Vec<ModVersion> = records
            .iter()
            .map(|x| ModVersion {
                id: x.version_id,
                name: x.name.clone(),
                description: x.description.clone(),
                version: x.version.clone(),
                download_link: x.download_link.clone(),
                managed_download_link: x.managed_download_link.clone(),
                download_count: x.mod_version_download_count.into(),
                hash: x.hash.clone(),
                geode: x.geode.clone(),
                early_load: x.early_load,
                requires_patching: x.requires_patching,
                api: x.api,
                status: x.status,
                mod_id: x.mod_id.clone(),
                gd: DetailedGDVersion {
                    win: None,
                    mac: None,
                    mac_arm: None,
                    mac_intel: None,
                    ios: None,
                    android: None,
                    android32: None,
                    android64: None,
                },
                developers: None,
                tags: None,
                dependencies: None,
                incompatibilities: None,
                created_at: x.mod_version_created_at,
                updated_at: x.mod_version_updated_at,
                direct_download_link: Some(x.download_link.clone()),
                info: x.info.clone(),
            })
            .collect();
        let ids: Vec<i32> = versions.iter().map(|x| x.id).collect();
        let gd: HashMap<i32, DetailedGDVersion> =
            ModGDVersion::get_for_mod_versions(&ids, pool).await?;
        let tags: Vec<String> = Tag::get_tags_for_mod(id, pool).await?;
        let devs: Vec<ModDeveloper> = developers::get_all_for_mod(id, pool).await?;
        let links: Option<ModLinks> = ModLinks::fetch(id, pool).await?;

        for i in &mut versions {
            let gd_versions = gd.get(&i.id).cloned().unwrap_or_default();
            i.gd = gd_versions;
        }

        let mod_entity = Mod {
            id: records[0].id.clone(),
            repository: records[0].repository.clone(),
            featured: records[0].featured,
            download_count: records[0].mod_download_count.into(),
            versions,
            tags,
            developers: devs,
            created_at: records[0].created_at,
            updated_at: records[0].updated_at,
            about: records[0].about.clone(),
            changelog: records[0].changelog.clone(),
            links,
        };
        Ok(Some(mod_entity))
    }

    /// At the moment this is only used to set the mod to featured.
    /// DOES NOT check if the mod exists
    #[tracing::instrument(skip_all, fields(mod_id = %id, featured = %featured))]
    pub async fn update_mod(
        id: &str,
        featured: bool,
        pool: &mut PgConnection,
    ) -> Result<(), DatabaseError> {
        sqlx::query!("UPDATE mods SET featured = $1 WHERE id = $2", featured, id)
            .execute(&mut *pool)
            .await
            .inspect_err(|e| tracing::error!("{:?}", e))
            .map_err(|e| e.into())
            .map(|_| ())
    }

    #[tracing::instrument(skip_all, fields(mod_ids = ?ids, platform = ?platforms, gd = ?gd))]
    pub async fn get_updates(
        ids: &[String],
        platforms: VerPlatform,
        geode: &semver::Version,
        gd: GDVersionEnum,
        pool: &mut PgConnection,
    ) -> Result<Vec<ModUpdate>, DatabaseError> {
        #[derive(sqlx::FromRow)]
        struct QueryResult {
            id: String,
            version: String,
            mod_version_id: i32,
        }

        let geode_pre = geode.pre.to_string();
        let geode_pre = (!geode_pre.is_empty()).then_some(geode_pre);

        let result = sqlx::query_as!(
            QueryResult,
            "SELECT
                q.id,
                q.inner_version as version,
                q.mod_version_id
            FROM (
                SELECT
                    m.id,
                    mv.id as mod_version_id,
                    mv.version as inner_version,
                    ROW_NUMBER() OVER (PARTITION BY m.id ORDER BY mv.id DESC) rn
                FROM mods m
                INNER JOIN mod_versions mv ON mv.mod_id = m.id
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                INNER JOIN mod_gd_versions mgv ON mv.id = mgv.mod_id
                WHERE mvs.status = 'accepted'
                AND mgv.platform = $1
                AND (mgv.gd = $2 OR mgv.gd = '*')
                AND m.id = ANY($3)
                AND $4 = mv.geode_major
                AND $5 >= mv.geode_minor
                AND (
                    ($7::text IS NULL AND mv.geode_meta NOT ILIKE 'alpha%')
                    OR (
                        $7 ILIKE 'alpha%'
                        AND $5 = mv.geode_minor
                        AND $6 = mv.geode_patch
                        AND $7 = mv.geode_meta
                    )
                    OR (
                        mv.geode_meta IS NULL
                        OR $5 > mv.geode_minor
                        OR $6 > mv.geode_patch
                        OR (mv.geode_meta NOT ILIKE 'alpha%' AND $7 >= mv.geode_meta)
                    )
                )
            ) q
            WHERE q.rn = 1",
            platforms as VerPlatform,
            gd as GDVersionEnum,
            ids,
            i32::try_from(geode.major).unwrap_or_default(),
            i32::try_from(geode.minor).unwrap_or_default(),
            i32::try_from(geode.patch).unwrap_or_default(),
            geode_pre
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?;

        if result.is_empty() {
            return Ok(vec![]);
        }

        // Client doesn't actually use those, we might as well not return them yet
        // TODO: enable back when client supports then
        // let ids: Vec<i32> = result.iter().map(|x| x.mod_version_id).collect();
        // let deps: HashMap<i32, Vec<FetchedDependency>> =
        //     Dependency::get_for_mod_versions(&ids, Some(platforms), Some(gd), Some(geode), pool).await?;
        // let incompat: HashMap<i32, Vec<FetchedIncompatibility>> =
        //     Incompatibility::get_for_mod_versions(&ids, Some(platforms), Some(gd), Some(geode), pool).await?;

        let mut ret: Vec<ModUpdate> = vec![];

        for r in result {
            let update = ModUpdate {
                id: r.id.clone(),
                version: r.version,
                mod_version_id: r.mod_version_id,
                download_link: "".to_string(),
                dependencies: vec![],
                incompatibilities: vec![],
                // dependencies: deps
                //     .get(&r.mod_version_id)
                //     .cloned()
                //     .unwrap_or_default()
                //     .iter()
                //     .map(|x| x.to_response())
                //     .collect(),
                // incompatibilities: incompat
                //     .get(&r.mod_version_id)
                //     .cloned()
                //     .unwrap_or_default()
                //     .iter()
                //     .map(|x| x.to_response())
                //     .collect(),
                replacement: None,
            };
            ret.push(update);
        }

        Ok(ret)
    }
}
