use super::{
    dependency::ResponseDependency,
    developer::{Developer, ModDeveloper},
    incompatibility::{Replacement, ResponseIncompatibility},
    mod_gd_version::{DetailedGDVersion, GDVersionEnum, ModGDVersion, VerPlatform},
    mod_link::ModLinks,
    tag::Tag,
};
use crate::database::repository::{developers, mods};
use crate::{
    endpoints::{
        developers::{SimpleDevMod, SimpleDevModVersion},
        mods::{IndexQueryParams, IndexSortType},
    },
    types::{
        api::{ApiError, PaginatedData},
        mod_json::{self, ModJson, ModJsonLinks},
        models::{mod_version::ModVersion, mod_version_status::ModVersionStatusEnum},
    },
};
use actix_web::web::Bytes;
use chrono::SecondsFormat;
use reqwest::Client;
use semver::Version;
use serde::Serialize;
use sqlx::{
    types::chrono::{DateTime, Utc},
    PgConnection, Postgres, QueryBuilder,
};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    str::FromStr,
};

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

pub enum CheckExistingResult {
    Exists,
    NotExists,
    ExistsNotValidated,
    ExistsWithRejected,
}

pub struct ModStats {
    pub total_count: i64,
    pub total_downloads: i64,
}

impl Mod {
    pub async fn get_stats(pool: &mut PgConnection) -> Result<ModStats, ApiError> {
        match sqlx::query!(
            "
            SELECT COUNT(id), SUM(download_count)
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
        {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(r) => {
                if let Some((Some(total_count), Some(total_downloads))) =
                    r.map(|o| (o.count, o.sum))
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
        }
    }

    pub async fn get_index(
        pool: &mut PgConnection,
        query: IndexQueryParams,
    ) -> Result<PaginatedData<Mod>, ApiError> {
        let tags = match query.tags {
            Some(t) => Tag::parse_tags(&t, pool).await?,
            None => vec![],
        };
        let page: i64 = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

        let limit = per_page;
        let offset = (page - 1) * per_page;
        let mut platforms: Vec<VerPlatform> = vec![];
        if query.platforms.is_some() {
            for i in query.platforms.unwrap().split(',') {
                let trimmed = i.trim();
                let platform = VerPlatform::from_str(trimmed).or(Err(ApiError::BadRequest(
                    format!("Invalid platform {}", trimmed),
                )))?;
                match platform {
                    VerPlatform::Android => {
                        platforms.push(VerPlatform::Android32);
                        platforms.push(VerPlatform::Android64);
                    }
                    VerPlatform::Mac => {
                        platforms.push(VerPlatform::MacArm);
                        platforms.push(VerPlatform::MacIntel);
                    }
                    _ => platforms.push(platform),
                }
            }
        }
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT q.id, q.repository, q.about, q.changelog, q.download_count, q.featured, q.created_at, q.updated_at, q.status
            FROM (SELECT m.id, m.repository, m.about, m.changelog, m.download_count, m.featured, m.created_at, m.updated_at, mvs.status,
            row_number() over (partition by m.id order by mv.id desc) rn FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id "#,
        );
        let mut counter_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT COUNT(DISTINCT m.id) FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id ",
        );

        if !tags.is_empty() {
            let sql = "INNER JOIN mods_mod_tags mmt ON mmt.mod_id = m.id ";
            builder.push(sql);
            counter_builder.push(sql);
        }

        if query.developer.is_some() {
            let sql = "INNER JOIN mods_developers md ON md.mod_id = m.id ";
            builder.push(sql);
            counter_builder.push(sql);
        }

        builder.push("WHERE ");
        counter_builder.push("WHERE ");

        if !tags.is_empty() {
            let sql = "mmt.tag_id = ANY(";
            builder.push(sql);
            counter_builder.push(sql);

            builder.push_bind(&tags);
            counter_builder.push_bind(&tags);
            let sql = ") AND ";
            builder.push(sql);
            counter_builder.push(sql);
        }

        if let Some(f) = query.featured {
            let sql = "m.featured = ";
            builder.push(sql);
            counter_builder.push(sql);
            builder.push_bind(f);
            counter_builder.push_bind(f);
            let sql = " AND ";
            builder.push(sql);
            counter_builder.push(sql);
        }

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

        if let Some(d) = developer {
            let sql = "md.developer_id = ";
            builder.push(sql);
            counter_builder.push(sql);
            builder.push_bind(d.id);
            counter_builder.push_bind(d.id);
            let sql = " AND ";
            builder.push(sql);
            counter_builder.push(sql);
        }

        let sql = "mvs.status = ";
        builder.push(sql);
        counter_builder.push(sql);

        let status = query.status.unwrap_or(ModVersionStatusEnum::Accepted);
        builder.push_bind(status);
        counter_builder.push_bind(status);

        let sql = " AND mv.name ILIKE ";
        builder.push(sql);
        counter_builder.push(sql);

        let query_string = format!("%{}%", query.query.unwrap_or("".to_string()).to_lowercase());
        counter_builder.push_bind(&query_string);
        builder.push_bind(&query_string);

        if let Some(ref geode) = query.geode {
            let geode = geode.trim_start_matches('v').to_string();
            if let Ok(parsed) = Version::parse(&geode) {
                let major = i32::try_from(parsed.major).unwrap_or_default();
                let minor = i32::try_from(parsed.minor).unwrap_or_default();
                let patch = i32::try_from(parsed.patch).unwrap_or_default();
                let meta = parsed.pre.to_string();
                let meta = (!meta.is_empty()).then_some(meta);

                // Always match major version
                let sql = " AND mv.geode_major = ";
                builder.push(sql);
                counter_builder.push(sql);
                builder.push_bind(major);
                counter_builder.push_bind(major);

                // If alpha, match exactly that version
                if parsed.pre.contains("alpha") {
                    let sql = " AND mv.geode_minor = ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(minor);
                    counter_builder.push_bind(minor);
                    let sql = " AND mv.geode_patch = ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(patch);
                    counter_builder.push_bind(patch);
                    let sql = " AND mv.geode_meta = ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(meta.clone());
                    counter_builder.push_bind(meta.clone());
                } else {
                    let sql = " AND mv.geode_minor <= ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(minor);
                    counter_builder.push_bind(minor);

                    let sql = " AND (mv.geode_meta IS NULL OR mv.geode_minor < ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(minor);
                    counter_builder.push_bind(minor);

                    let sql = " OR mv.geode_patch < ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(patch);
                    counter_builder.push_bind(patch);

                    let sql = " OR (mv.geode_meta NOT ILIKE 'alpha%' AND mv.geode_meta <= ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(meta.clone());
                    counter_builder.push_bind(meta.clone());

                    let sql = "))";
                    builder.push(sql);
                    counter_builder.push(sql);
                }
            }
        }

        if let Some(g) = query.gd {
            let sql = " AND (mgv.gd = ";
            builder.push(sql);
            builder.push_bind(g);
            counter_builder.push(sql);
            counter_builder.push_bind(g);
            let sql = " OR mgv.gd = ";
            builder.push(sql);
            counter_builder.push(sql);
            builder.push_bind(GDVersionEnum::All);
            counter_builder.push_bind(GDVersionEnum::All);
            let sql = ")";
            builder.push(sql);
            counter_builder.push(sql);
        }

        for (i, platform) in platforms.iter().enumerate() {
            if i == 0 {
                let sql = " AND mgv.platform IN (";
                builder.push(sql);
                counter_builder.push(sql);
            }
            builder.push_bind(*platform);
            counter_builder.push_bind(*platform);
            if i == platforms.len() - 1 {
                builder.push(")");
                counter_builder.push(")");
            } else {
                builder.push(", ");
                counter_builder.push(", ");
            }
        }

        match query.sort {
            IndexSortType::Downloads => {
                builder.push(" ORDER BY m.download_count DESC");
            }
            IndexSortType::RecentlyUpdated => {
                builder.push(" ORDER BY m.updated_at DESC");
            }
            IndexSortType::RecentlyPublished => {
                builder.push(" ORDER BY m.created_at DESC");
            }
            IndexSortType::Name => {
                builder.push(" ORDER BY mv.name ASC");
            }
            IndexSortType::NameReverse => {
                builder.push(" ORDER BY mv.name DESC");
            }
        }

        builder.push(") q WHERE q.rn = 1 LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let result = builder
            .build_query_as::<ModRecord>()
            .fetch_all(&mut *pool)
            .await;
        let records = match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        let result = counter_builder
            .build_query_scalar()
            .fetch_one(&mut *pool)
            .await;
        let count = match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(c) => c,
        };

        if records.is_empty() {
            return Ok(PaginatedData {
                data: vec![],
                count,
            });
        }

        if status == ModVersionStatusEnum::Pending {
            return Mod::get_pending(records, count, pool).await;
        }

        let ids: Vec<_> = records.iter().map(|x| x.id.clone()).collect();
        let versions = ModVersion::get_latest_for_mods(
            pool,
            ids.clone(),
            query.gd,
            platforms,
            query.geode.as_ref(),
        )
        .await?;
        let developers = developers::get_all_for_mods(&ids, pool).await?;
        let links = ModLinks::fetch_for_mods(&ids, pool).await?;
        let mut mod_version_ids: Vec<i32> = vec![];
        for (_, mod_version) in versions.iter() {
            mod_version_ids.push(mod_version.id);
        }

        let gd_versions = ModGDVersion::get_for_mod_versions(&mod_version_ids, pool).await?;
        let tags = Tag::get_tags_for_mods(&ids, pool).await?;

        let ret = records
            .into_iter()
            .map(|x| {
                let mut version = versions.get(&x.id).cloned().unwrap();
                let gd_ver = gd_versions.get(&version.id).cloned().unwrap_or_default();
                version.gd = gd_ver;

                let devs = developers.get(&x.id).cloned().unwrap_or_default();
                let tags = tags.get(&x.id).cloned().unwrap_or_default();
                let links = links.iter().find(|link| link.mod_id == x.id).cloned();

                Mod {
                    id: x.id.clone(),
                    repository: x.repository.clone(),
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
        pool: &mut PgConnection,
    ) -> Result<Vec<SimpleDevMod>, ApiError> {
        #[derive(sqlx::FromRow)]
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

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT
            m.id, m.featured, m.download_count as mod_download_count,
            mv.name, mv.version, mv.download_count as mod_version_download_count,
            mvs.info, mvs.status,
            exists(
                select 1 from mod_version_statuses mvs_inner
                where mvs_inner.mod_version_id = mv.id and mvs_inner.status = 'accepted'
            ) as validated
            FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mods_developers md ON md.mod_id = m.id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE md.developer_id = ",
        );
        query_builder.push_bind(id);
        query_builder.push(" AND mvs.status = ");
        query_builder.push_bind(status);

        let records = match query_builder
            .build_query_as::<Record>()
            .fetch_all(&mut *pool)
            .await
        {
            Ok(e) => e,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

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
    ) -> Result<Option<Mod>, ApiError> {
        let records: Vec<ModRecordGetOne> = sqlx::query_as(
            r#"SELECT
                m.id, m.repository, m.about, m.changelog, m.featured, m.download_count as mod_download_count, m.created_at, m.updated_at,
                mv.id as version_id, mv.name, mv.description, mv.version, mv.download_link, mv.download_count as mod_version_download_count,
                mv.created_at as mod_version_created_at, mv.updated_at as mod_version_updated_at,
                mv.hash,
                format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as geode,
                mv.early_load, mv.api, mv.mod_id, mvs.status, mvs.info
            FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE m.id = $1
            AND ($2 = false OR mvs.status = 'accepted')
            ORDER BY mv.id DESC"#,
        )
            .bind(id)
            .bind(only_accepted)
            .fetch_all(&mut *pool)
            .await
            .or(Err(ApiError::DbError))?;
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

    pub async fn from_json(
        json: &ModJson,
        developer: Developer,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if Version::parse(json.version.trim_start_matches('v')).is_err() {
            return Err(ApiError::BadRequest(format!(
                "Invalid mod version semver {}",
                json.version
            )));
        };

        if Version::parse(json.geode.trim_start_matches('v')).is_err() {
            return Err(ApiError::BadRequest(format!(
                "Invalid geode version semver {}",
                json.geode
            )));
        };

        Mod::create(json, developer, pool).await?;
        if let Some(l) = &json.links {
            if l.community.is_some() || l.homepage.is_some() || l.source.is_some() {
                ModLinks::upsert_for_mod(
                    &json.id,
                    l.community.clone(),
                    l.homepage.clone(),
                    l.source.clone(),
                    pool,
                )
                .await?;
            }
        }
        ModVersion::create_from_json(json, false, pool).await?;
        Ok(())
    }

    pub async fn new_version(
        json: &ModJson,
        developer: &Developer,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let result = sqlx::query!(
            "SELECT DISTINCT m.id FROM mods m
            INNER JOIN mod_versions mv ON mv.mod_id = m.id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE m.id = $1",
            json.id
        )
        .fetch_optional(&mut *pool)
        .await
        .or(Err(ApiError::DbError))?;
        if result.is_none() {
            return Err(ApiError::NotFound(format!(
                "Mod {} doesn't exist",
                &json.id
            )));
        }

        struct ModVersionItem {
            version: String,
            id: i32,
            status: ModVersionStatusEnum,
        }

        let latest = match sqlx::query_as!(
            ModVersionItem,
            r#"SELECT mv.version, mv.id, mvs.status as "status!: ModVersionStatusEnum" FROM mod_versions mv
            INNER JOIN mod_version_statuses mvs ON mv.status_id = mvs.id
            WHERE mv.mod_id = $1
            AND (mvs.status = 'pending' OR mvs.status = 'accepted' OR mvs.status = 'rejected')
            ORDER BY mv.id DESC
            LIMIT 1"#,
            &json.id
        )
            .fetch_one(&mut *pool)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::info!("Failed to fetch latest version for mod. Error: {}", e);
                return Err(ApiError::DbError);
            }
        };

        let version = Version::parse(&latest.version).map_err(|_| {
            log::error!(
                "Invalid semver for locally stored version: id {}, version {}",
                latest.id,
                latest.version
            );
            ApiError::InternalError
        })?;
        let new_version = Version::parse(json.version.trim_start_matches('v'))
            .map_err(|_| ApiError::BadRequest(format!("Invalid semver {}", json.version)))?;
        if new_version <= version {
            return Err(ApiError::BadRequest(format!(
                "mod.json version {} is smaller / equal to latest mod version {}",
                json.version, version
            )));
        }

        let accepted_versions = ModVersion::get_accepted_count(&json.id, &mut *pool).await?;

        let verified = match accepted_versions {
            0 => false,
            _ => developer.verified,
        };

        if latest.status == ModVersionStatusEnum::Pending {
            ModVersion::update_pending_version(latest.id, json, verified, pool).await?;
        } else {
            ModVersion::create_from_json(json, verified, pool).await?;
        }

        Mod::update_existing_with_json(json, verified, pool).await?;

        Ok(())
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

    async fn create(
        json: &ModJson,
        developer: Developer,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let updated: bool = match Mod::check_for_existing(&json.id, pool).await? {
            CheckExistingResult::Exists => {
                return Err(ApiError::BadRequest(format!(
                    "Mod {} already exists, consider creating a new version",
                    json.id
                )))
            }
            CheckExistingResult::ExistsNotValidated => {
                return Err(ApiError::BadRequest(format!(
                    "Mod {} already exists, but has not been accepted by an index admin",
                    json.id
                )))
            }
            CheckExistingResult::ExistsWithRejected => {
                if !developers::has_access_to_mod(developer.id, &json.id, pool).await? {
                    return Err(ApiError::Forbidden);
                }
                Mod::update_existing_with_json(json, developer.verified, pool).await?;

                if let Err(e) = sqlx::query!(
                    "DELETE FROM mod_versions mv
                    USING mod_version_statuses mvs
                    WHERE mv.id = mvs.mod_version_id
                        AND mv.mod_id = $1
                        AND mvs.status = 'rejected'",
                    &json.id
                )
                .execute(&mut *pool)
                .await
                {
                    log::error!(
                        "Failed to remove existing rejected versions from mod {}: {}",
                        json.id,
                        e
                    );
                    return Err(ApiError::DbError);
                }

                true
            }
            CheckExistingResult::NotExists => false,
        };

        if updated {
            return Ok(());
        }

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO mods (");
        if json.repository.is_some() {
            query_builder.push("repository, ");
        }
        if json.changelog.is_some() {
            query_builder.push("changelog, ");
        }
        if json.about.is_some() {
            query_builder.push("about, ");
        }
        query_builder.push("id, image) VALUES (");
        let mut separated = query_builder.separated(", ");
        if let Some(repo) = &json.repository {
            separated.push_bind(repo);
        }
        if let Some(changelog) = &json.changelog {
            separated.push_bind(changelog);
        }
        if let Some(about) = &json.about {
            separated.push_bind(about);
        }
        separated.push_bind(&json.id);
        separated.push_bind(&json.logo);
        separated.push_unseparated(")");

        if let Err(e) = query_builder.build().execute(&mut *pool).await {
            log::error!("Failed to insert mod {} into database: {}", json.id, e);
            return Err(ApiError::DbError);
        }
        Mod::assign_owner(&json.id, developer.id, pool).await?;

        Ok(())
    }

    async fn check_for_existing(
        id: &str,
        pool: &mut PgConnection,
    ) -> Result<CheckExistingResult, ApiError> {
        struct Counts {
            not_rejected: i64,
            rejected: i64,
            validated: i64,
        }

        let row = sqlx::query!("SELECT id FROM mods WHERE id = $1", id)
            .fetch_optional(&mut *pool)
            .await
            .map_err(|e| {
                log::error!("Failed to fetch existing mod {}: {}", id, e);
                ApiError::DbError
            })?;

        if row.is_none() {
            return Ok(CheckExistingResult::NotExists);
        }

        let counts = sqlx::query!(
            "SELECT
            COUNT(1) FILTER (WHERE mvs.status = ANY(ARRAY['accepted', 'pending']::mod_version_status[])) AS not_rejected,
            COUNT(1) FILTER (WHERE mvs.status = 'rejected') AS rejected,
            COUNT(1) FILTER (WHERE mvs.status = 'accepted') AS validated
            FROM mod_versions mv
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE mv.mod_id = $1",
            id
        )
            .fetch_one(&mut *pool)
            .await
            .map(|row| Counts {
                validated: row.validated.unwrap_or(0),
                not_rejected: row.not_rejected.unwrap_or(0),
                rejected: row.rejected.unwrap_or(0)
            })
            .map_err(|e| {
                log::error!("Failed to fetch version counts for mod {}: {}", id, e);
                ApiError::DbError
            })?;

        if counts.validated > 0 {
            return Ok(CheckExistingResult::Exists);
        }
        if counts.validated == 0 && counts.not_rejected > 0 {
            return Ok(CheckExistingResult::ExistsNotValidated);
        }
        if counts.rejected > 0 {
            return Ok(CheckExistingResult::ExistsWithRejected);
        }

        // Mod exists with no uploaded versions, very rare
        Ok(CheckExistingResult::NotExists)
    }

    async fn update_existing_with_json(
        json: &ModJson,
        update_timestamp: bool,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE mods SET ");
        if json.repository.is_some() {
            query_builder.push("repository = ");
            query_builder.push_bind(&json.repository);
            query_builder.push(", ");
        }
        if json.changelog.is_some() {
            query_builder.push("changelog = ");
            query_builder.push_bind(&json.changelog);
            query_builder.push(", ");
        }
        if json.about.is_some() {
            query_builder.push("about = ");
            query_builder.push_bind(&json.about);
        }
        if !json.logo.is_empty() {
            query_builder.push(", ");
            query_builder.push("image = ");
            query_builder.push_bind(&json.logo);
        }
        query_builder.push(" WHERE id = ");
        query_builder.push_bind(&json.id);

        if let Err(e) = query_builder.build().execute(&mut *pool).await {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }

        if update_timestamp {
            match sqlx::query!(
                "update mods m
                set updated_at = $1
                where id = $2",
                Utc::now(),
                &json.id
            )
            .execute(&mut *pool)
            .await
            {
                Err(e) => {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                }
                Ok(r) => {
                    if r.rows_affected() == 0 {
                        log::error!("Couldn't update timestamp on mod. No rows affected.");
                    }
                }
            }
        }

        let links = ModLinks::fetch(&json.id, pool).await?;

        if links.is_some() || json.links.is_some() {
            let links = json.links.clone().unwrap_or(ModJsonLinks {
                community: None,
                source: None,
                homepage: None,
            });
            ModLinks::upsert_for_mod(
                &json.id,
                links.community,
                links.homepage,
                links.source,
                pool,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn assign_owner(
        mod_id: &str,
        dev_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        sqlx::query!(
            "UPDATE mods_developers
            SET is_owner = false
            WHERE mod_id = $1",
            mod_id
        )
        .execute(&mut *pool)
        .await
        .map_err(|e| {
            log::error!("Failed to unassign owner from mod {}: {}", mod_id, e);
            ApiError::DbError
        })?;

        struct FetchedRow {
            developer_id: i32,
            is_owner: bool,
        }

        match sqlx::query_as!(
            FetchedRow,
            "SELECT
                md.developer_id,
                md.is_owner
            FROM mods_developers md
            WHERE md.mod_id = $1
            AND md.developer_id = $2",
            mod_id,
            dev_id
        )
        .fetch_optional(&mut *pool)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to fetch existing developer for mod {}: {}",
                mod_id,
                e
            );
            ApiError::DbError
        })? {
            None => {
                sqlx::query!(
                    "INSERT INTO mods_developers (mod_id, developer_id, is_owner) VALUES
                    ($1, $2, true)",
                    mod_id,
                    dev_id
                )
                .execute(&mut *pool)
                .await
            }
            Some(_) => {
                sqlx::query!(
                    "UPDATE mods_developers SET is_owner = true
                    WHERE mod_id = $1 AND developer_id = $2",
                    mod_id,
                    dev_id
                )
                .execute(&mut *pool)
                .await
            }
        }
        .map_err(|e| {
            log::error!("Failed to assign owner {} to mod {}: {}", dev_id, mod_id, e);
            ApiError::DbError
        })?;
        Ok(())
    }

    pub async fn assign_dev(
        mod_id: &str,
        dev_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        struct FetchedRow {
            developer_id: i32,
            is_owner: bool,
        }

        let assignment = sqlx::query_as!(
            FetchedRow,
            "SELECT md.developer_id, md.is_owner FROM mods_developers md
            WHERE md.mod_id = $1
            AND md.developer_id = $2",
            mod_id,
            dev_id
        )
        .fetch_optional(&mut *pool)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to fetch existing dev for assignment on mod {}: {}",
                mod_id,
                e
            );
            ApiError::DbError
        })?;
        if assignment.is_some() {
            return Err(ApiError::BadRequest(format!(
                "This developer is already assigned on mod {}",
                mod_id
            )));
        }

        sqlx::query!(
            "INSERT INTO mods_developers (mod_id, developer_id)
            VALUES ($1, $2)",
            mod_id,
            dev_id
        )
        .execute(&mut *pool)
        .await
        .map_err(|e| {
            log::error!("Couldn't add new developer to mod {}: {}", mod_id, e);
            ApiError::DbError
        })?;
        Ok(())
    }

    pub async fn unassign_dev(
        mod_id: &str,
        dev_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        struct FetchedRow {
            developer_id: i32,
            is_owner: bool,
        }

        let existing = sqlx::query_as!(
            FetchedRow,
            "SELECT md.developer_id, md.is_owner FROM mods_developers md
            WHERE md.mod_id = $1
            AND md.developer_id = $2",
            mod_id,
            dev_id
        )
        .fetch_optional(&mut *pool)
        .await
        .map_err(|err| {
            log::error!("Failed to fetch existing developers: {}", err);
            ApiError::DbError
        })?
        .ok_or(ApiError::NotFound(
            "Developer is not assigned to mod".into(),
        ))?;

        if existing.is_owner {
            return Err(ApiError::BadRequest(
                "Cannot unassign the owner developer for the mod".to_string(),
            ));
        }

        sqlx::query!(
            "DELETE FROM mods_developers
            WHERE mod_id = $1 AND developer_id = $2",
            mod_id,
            dev_id
        )
        .execute(&mut *pool)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to remove assigned developer {} from mod {}: {}",
                dev_id,
                mod_id,
                e
            );
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

    pub async fn update_mod_image(
        id: &str,
        hash: &str,
        download_link: &str,
        limit_mb: u32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let mut cursor = download_geode_file(download_link, limit_mb).await?;
        let mut bytes: Vec<u8> = vec![];
        cursor.read_to_end(&mut bytes).map_err(|e| {
            log::error!("Failed to fetch .geode for updating mod image: {}", e);
            ApiError::InternalError
        })?;

        let new_hash = sha256::digest(bytes);
        if new_hash != hash {
            return Err(ApiError::BadRequest(format!(
                "Different hash detected: old: {}, new: {}",
                hash, new_hash
            )));
        }

        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| {
            log::error!("Failed to create ZipArchive for .geode: {}", e);
            ApiError::BadRequest("Couldn't unzip .geode file".to_string())
        })?;

        let image_file = archive.by_name("logo.png").ok();
        if image_file.is_none() {
            return Ok(());
        }
        let mut image_file = image_file.unwrap();

        let image = mod_json::validate_mod_logo(&mut image_file, true)?;

        sqlx::query!(
            "UPDATE mods SET image = $1
            WHERE id = $2",
            image,
            id
        )
        .execute(&mut *pool)
        .await
        .map_err(|e| {
            log::error!("{}", e);
            ApiError::DbError
        })?;

        Ok(())
    }
}

pub async fn download_geode_file(url: &str, limit_mb: u32) -> Result<Cursor<Bytes>, ApiError> {
    let limit_bytes = limit_mb * 1_000_000;
    let size = get_download_size(url).await?;
    if size > limit_bytes as u64 {
        return Err(ApiError::BadRequest(format!(
            "File size is too large, max {}MB",
            limit_mb
        )));
    }
    Ok(Cursor::new(
        reqwest::get(url)
            .await
            .map_err(|e| {
                log::error!("Failed to fetch .geode: {}", e);
                ApiError::BadRequest("Couldn't download .geode file".into())
            })?
            .bytes()
            .await
            .map_err(|e| {
                log::error!("Failed to get bytes from .geode: {}", e);
                ApiError::InternalError
            })?,
    ))
}

async fn get_download_size(url: &str) -> Result<u64, ApiError> {
    let res = Client::new().head(url).send().await.map_err(|err| {
        log::error!("Failed to send HEAD request for .geode filesize: {:?}", err);
        ApiError::BadRequest("Failed to query filesize for given URL".into())
    })?;

    res.headers()
        .get("content-length")
        .ok_or(ApiError::BadRequest(
            "Couldn't extract download size from URL".into(),
        ))?
        .to_str()
        .map_err(|_| ApiError::BadRequest("Invalid Content-Length for .geode".into()))?
        .parse::<u64>()
        .map_err(|_| ApiError::BadRequest("Invalid Content-Length for .geode".into()))
}
