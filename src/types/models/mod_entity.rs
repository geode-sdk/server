use super::{
    dependency::ResponseDependency,
    developer::ModDeveloper,
    incompatibility::{Replacement, ResponseIncompatibility},
    mod_gd_version::{DetailedGDVersion, GDVersionEnum, ModGDVersion, VerPlatform},
    mod_link::ModLinks,
    tag::Tag,
};
use crate::{
    database::{
        repository::{developers, mods},
        DatabaseError,
    },
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
    },
};
use chrono::SecondsFormat;
use semver::Version;
use serde::Serialize;
use sqlx::{
    types::chrono::{DateTime, Utc},
    PgConnection,
};
use std::{collections::HashMap, str::FromStr};

#[derive(Serialize, Debug, sqlx::FromRow)]
pub struct Mod {
    pub id: String,
    pub repository: Option<String>,
    pub featured: bool,
    pub download_count: i32,
    pub developers: Vec<ModDeveloper>,
    pub versions: Vec<ModVersion>,
    pub tags: Vec<String>,
    pub about: Option<String>,
    pub changelog: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub links: Option<ModLinks>,
}

#[derive(Serialize, Debug)]
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
    mod_version_download_count: i32,
    hash: String,
    geode: String,
    early_load: bool,
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
        .inspect_err(|e| log::error!("failed to get mod stats: {}", e))?;

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

    pub async fn get_index(
        pool: &mut PgConnection,
        query: IndexQueryParams,
    ) -> Result<PaginatedData<Mod>, ApiError> {
        let tags = match query.tags {
            Some(t) => Some(Tag::parse_tags(&t, pool).await?),
            None => None,
        };
        let page: i64 = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

        let limit = per_page;
        let offset = (page - 1) * per_page;
        let platforms = query
            .platforms
            .map(|p| VerPlatform::parse_query_string(&p))
            .transpose()?;
        let status = query.status.unwrap_or(ModVersionStatusEnum::Accepted);

        let developer = match query.developer {
            Some(d) => match developers::get_one_by_username(&d, pool).await? {
                Some(d) => Some(d),
                None => {
                    return Ok(PaginatedData {
                        data: vec![],
                        count: 0,
                    })
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
        };

        let geode = query
            .geode
            .map(|x| Version::parse(&x))
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

        // VERY IMPORTANT MESSAGE BELOW.
        // This beautiful chunk of code below uses format!() to reuse the same joins / where clauses
        // in 2 queries. This uses prepared statements, the parameters are bound in the queries at the end.
        //
        // DO NOT, I repeat, DO NOT enter any user input inside the format!().
        // I will find you personally if you do so.
        //
        // - Flame

        let joins_filters = r#"
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id
            LEFT JOIN mods_mod_tags mmt ON mmt.mod_id = m.id
            INNER JOIN mods_developers md ON md.mod_id = m.id
            WHERE ($1 IS NULL OR mmt.tag_id = ANY($1))
            AND ($2 IS NULL OR m.featured = $2)
            AND ($3 IS NULL OR md.developer_id = $3)
            AND ($13 IS NULL OR mvs.status = $13)
            AND ($4 IS NULL OR $4 = mv.geode_major)
            AND ($5 IS NULL OR $5 >= mv.geode_minor)
            AND (
                ($7 IS NULL AND mv.geode_meta NOT ILIKE 'alpha%')
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
            AND ($8 IS NULL OR mv.name ILIKE '%' || $8 || '%' OR m.id = $8)
            AND ($9 IS NULL OR mgv.gd = ANY($9))
            AND ($10 IS NULL OR mgv.platform = ANY($10))
        "#;

        let records: Vec<ModRecord> = sqlx::query_as(&format!(
            "SELECT q.id, q.repository, q.about, q.changelog,
                q.download_count, q.featured, q.created_at, q.updated_at
            FROM (
                SELECT m.id, mv.name, m.repository, m.about, m.changelog,
                    m.download_count, m.featured, m.created_at, m.updated_at,
                    ROW_NUMBER() OVER (PARTITION BY m.id ORDER BY mv.id DESC) rn
                FROM mods m
                {}
            ) q
            WHERE q.rn = 1
            ORDER BY {}
            LIMIT $11
            OFFSET $12
            ",
            joins_filters, order
        ))
        .bind(tags.as_ref())
        .bind(query.featured)
        .bind(developer.as_ref().map(|x| x.id))
        .bind(geode_major)
        .bind(geode_minor)
        .bind(geode_patch)
        .bind(geode_meta.as_ref())
        .bind(query.query.as_ref())
        .bind(gd.as_ref())
        .bind(platforms.as_ref())
        .bind(limit)
        .bind(offset)
        .bind(status)
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| log::error!("Failed to fetch mod index: {}", e))?;

        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(DISTINCT m.id)
            FROM mods m
            {}",
            joins_filters
        ))
        .bind(&tags)
        .bind(query.featured)
        .bind(developer.as_ref().map(|x| x.id))
        .bind(geode_major)
        .bind(geode_minor)
        .bind(geode_patch)
        .bind(&geode_meta)
        .bind(&query.query)
        .bind(&gd)
        .bind(&platforms)
        .bind(limit)
        .bind(offset)
        .bind(status)
        .fetch_optional(&mut *pool)
        .await
        .inspect_err(|e| log::error!("Failed to fetch mod index count: {}", e))?
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
        )
        .await?;
        let mut developers = developers::get_all_for_mods(&ids, pool).await?;
        let links = ModLinks::fetch_for_mods(&ids, pool).await?;
        let mod_version_ids: Vec<i32> = versions
            .iter()
            .map(|(_, mod_version)| mod_version.id)
            .collect();

        let mut gd_versions = ModGDVersion::get_for_mod_versions(&mod_version_ids, pool).await?;
        let mut tags = Tag::get_tags_for_mods(&ids, pool).await?;

        let ret = records
            .into_iter()
            .map(|x| {
                let mut version = versions.remove(&x.id).unwrap();
                version.gd = gd_versions.remove(&version.id).unwrap_or_default();

                let devs = developers.remove(&x.id).unwrap_or_default();
                let tags = tags.remove(&x.id).unwrap_or_default();
                let links = links.iter().find(|link| link.mod_id == x.id).cloned();

                Mod {
                    id: x.id,
                    repository: x.repository,
                    download_count: x.download_count,
                    featured: x.featured,
                    versions: vec![version],
                    tags,
                    developers: devs,
                    created_at: x.created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
                    updated_at: x.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
                    about: None,
                    changelog: None,
                    links,
                }
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
                    download_count: x.download_count,
                    featured: x.featured,
                    versions: version,
                    tags,
                    developers: devs,
                    created_at: x.created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
                    updated_at: x.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
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
        .inspect_err(|x| log::error!("Failed to fetch developer mods: {}", x))?;

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

    pub async fn get_one(
        id: &str,
        only_accepted: bool,
        pool: &mut PgConnection,
    ) -> Result<Option<Mod>, DatabaseError> {
        let records = sqlx::query_as!(
            ModRecordGetOne,
            r#"SELECT
                m.id, m.repository, m.about, m.changelog, m.featured, m.download_count as mod_download_count, m.created_at, m.updated_at,
                mv.id as version_id, mv.name, mv.description, mv.version, mv.download_link, mv.download_count as mod_version_download_count,
                mv.created_at as mod_version_created_at, mv.updated_at as mod_version_updated_at,
                mv.hash,
                format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as "geode!: _",
                mv.early_load, mv.api, mv.mod_id, mvs.status as "status: _", mvs.info
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
        .inspect_err(|e| log::error!("{}", e))?;

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
                download_count: x.mod_version_download_count,
                hash: x.hash.clone(),
                geode: x.geode.clone(),
                early_load: x.early_load,
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
                created_at: x
                    .mod_version_created_at
                    .map(|x| x.to_rfc3339_opts(SecondsFormat::Secs, true)),
                updated_at: x
                    .mod_version_updated_at
                    .map(|x| x.to_rfc3339_opts(SecondsFormat::Secs, true)),
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
            download_count: records[0].mod_download_count,
            versions,
            tags,
            developers: devs,
            created_at: records[0]
                .created_at
                .to_rfc3339_opts(SecondsFormat::Secs, true),
            updated_at: records[0]
                .updated_at
                .to_rfc3339_opts(SecondsFormat::Secs, true),
            about: records[0].about.clone(),
            changelog: records[0].changelog.clone(),
            links,
        };
        Ok(Some(mod_entity))
    }

    /// At the moment this is only used to set the mod to featured.
    /// Checks if the mod exists.
    pub async fn update_mod(
        id: &str,
        featured: bool,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if !mods::exists(id, &mut *pool).await? {
            return Err(ApiError::NotFound(format!("Mod {} doesn't exist", id)));
        }

        sqlx::query!("UPDATE mods SET featured = $1 WHERE id = $2", featured, id)
            .execute(&mut *pool)
            .await
            .map_err(|e| {
                log::error!("Failed to update mod {}: {}", id, e);
                ApiError::DbError
            })?;

        Ok(())
    }

    pub async fn get_updates(
        ids: &[String],
        platforms: VerPlatform,
        geode: &semver::Version,
        gd: GDVersionEnum,
        pool: &mut PgConnection,
    ) -> Result<Vec<ModUpdate>, ApiError> {
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
                AND (mgv.gd = ANY($2))
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
            &[GDVersionEnum::All, gd] as &[GDVersionEnum],
            ids,
            i32::try_from(geode.major).unwrap_or_default(),
            i32::try_from(geode.minor).unwrap_or_default(),
            i32::try_from(geode.patch).unwrap_or_default(),
            geode_pre
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|x| log::error!("Failed to fetch mod updates: {}", x))
        .or(Err(ApiError::DbError))?;

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
