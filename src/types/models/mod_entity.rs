use crate::{
    endpoints::{
        developers::{SimpleDevMod, SimpleDevModVersion},
        mods::{IndexQueryParams, IndexSortType},
    },
    types::{
        api::{ApiError, PaginatedData},
        mod_json::{self, ModJson},
        models::{
            mod_version::ModVersion, mod_version_status::ModVersionStatusEnum,
        },
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
use std::{collections::HashMap, io::{Cursor, Read}, str::FromStr};

use super::{
    dependency::ResponseDependency,
    developer::{Developer, FetchedDeveloper},
    incompatibility::{Replacement, ResponseIncompatibility},
    mod_gd_version::{DetailedGDVersion, GDVersionEnum, ModGDVersion, VerPlatform},
    tag::Tag,
};

#[derive(Serialize, Debug, sqlx::FromRow)]
pub struct Mod {
    pub id: String,
    pub repository: Option<String>,
    pub featured: bool,
    pub download_count: i32,
    pub developers: Vec<Developer>,
    pub versions: Vec<ModVersion>,
    pub tags: Vec<String>,
    pub about: Option<String>,
    pub changelog: Option<String>,
    pub created_at: String,
    pub updated_at: String,
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
    info: Option<String>,
}

pub enum CheckExistingResult {
    Exists,
    NotExists,
    ExistsNotValidated,
    ExistsWithRejected
}

pub struct ModStats {
    pub total_count: i64,
    pub total_downloads: i64,
}

impl Mod {
    pub async fn get_stats(pool: &mut PgConnection) -> Result<ModStats, ApiError> {
        match sqlx::query!("
            SELECT COUNT(id), SUM(download_count)
            FROM (
                select m.id, m.download_count, row_number() over(partition by m.id) rn
                FROM mods m
                INNER JOIN mod_versions mv ON mv.mod_id = m.id 
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                WHERE mvs.status = 'accepted'
            ) q
            WHERE q.rn = 1
        ")
        .fetch_optional(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(r) => if let Some((Some(total_count), Some(total_downloads))) = r.map(|o| (o.count, o.sum)) {
                Ok(ModStats { total_count, total_downloads })
            }
            else {
                Ok(ModStats { total_count: 0, total_downloads: 0 })
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
                    },
                    VerPlatform::Mac => {
                        platforms.push(VerPlatform::MacArm);
                        platforms.push(VerPlatform::MacIntel);
                    },
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
            INNER JOIN mod_gd_versions mgv ON mgv.mod_id = mv.id "#
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
            Some(d) => match Developer::find_by_username(&d, pool).await? {
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
                // If alpha, match exactly that version
                if parsed.pre.contains("alpha") {
                    let sql = " AND mv.geode = ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(parsed.to_string());
                    counter_builder.push_bind(parsed.to_string());
                } else {
                    let sql = " AND (SPLIT_PART(mv.geode, '.', 1) = ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(parsed.major.to_string());
                    counter_builder.push_bind(parsed.major.to_string());

                    let sql = " AND SPLIT_PART(mv.geode, '-', 2) NOT LIKE 'alpha%' AND SPLIT_PART(mv.geode, '.', 2) <= ";
                    builder.push(sql);
                    counter_builder.push(sql);
                    builder.push_bind(parsed.minor.to_string());
                    counter_builder.push_bind(parsed.minor.to_string());

                    // Match only higher betas (or no beta)
                    if parsed.pre.contains("beta") {
                        let sql = " AND (SPLIT_PART(mv.geode, '-', 2) = ''
                            OR SPLIT_PART(mv.geode, '-', 2) <=";
                        builder.push(sql);
                        counter_builder.push(sql);
                        builder.push_bind(parsed.pre.to_string());
                        counter_builder.push_bind(parsed.pre.to_string());
                        builder.push(")");
                        counter_builder.push(")");
                    }

                    builder.push(")");
                    counter_builder.push(")");
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
        let versions = ModVersion::get_latest_for_mods(pool, ids.clone(), query.gd, platforms, query.geode.as_ref()).await?;
        let developers = Developer::fetch_for_mods(&ids, pool).await?;
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
        let developers = Developer::fetch_for_mods(&ids, pool).await?;
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
            info: Option<String>
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
            WHERE md.developer_id = "
        );
        query_builder.push_bind(id);
        query_builder.push(" AND mvs.status = ");
        query_builder.push_bind(status);

        let records = match query_builder.build_query_as::<Record>()
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
                status: record.status
            };

            versions.entry(record.id.clone()).or_default().push(version);
        }
        let ids: Vec<String> = records.iter().map(|x| x.id.clone()).collect();
        let developers = Developer::fetch_for_mods(&ids, pool).await?;

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

    pub async fn get_one(id: &str, only_accepted: bool, pool: &mut PgConnection) -> Result<Option<Mod>, ApiError> {
        let records: Vec<ModRecordGetOne> = sqlx::query_as!(
            ModRecordGetOne,
            r#"SELECT
                m.id, m.repository, m.about, m.changelog, m.featured, m.download_count as mod_download_count, m.created_at, m.updated_at,
                mv.id as version_id, mv.name, mv.description, mv.version, mv.download_link, mv.download_count as mod_version_download_count,
                mv.hash, mv.geode, mv.early_load, mv.api, mv.mod_id, mvs.status as "status: _", mvs.info
            FROM mods m
            INNER JOIN mod_versions mv ON m.id = mv.mod_id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE m.id = $1 
            AND ($2 = false OR mvs.status = 'accepted')"#,
            id,
            only_accepted
        )
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
                direct_download_link: Some(x.download_link.clone()),
                info: Some(x.info.clone()),
            })
            .collect();
        let ids = versions.iter().map(|x| x.id).collect();
        let gd = ModGDVersion::get_for_mod_versions(&ids, pool).await?;
        let tags = Tag::get_tags_for_mod(id, pool).await?;
        let devs = Developer::fetch_for_mod(id, pool).await?;

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
        };
        Ok(Some(mod_entity))
    }

    pub async fn from_json(
        json: &ModJson,
        developer: FetchedDeveloper,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if semver::Version::parse(json.version.trim_start_matches('v')).is_err() {
            return Err(ApiError::BadRequest(format!(
                "Invalid mod version semver {}",
                json.version
            )));
        };

        if semver::Version::parse(json.geode.trim_start_matches('v')).is_err() {
            return Err(ApiError::BadRequest(format!(
                "Invalid geode version semver {}",
                json.geode
            )));
        };
        let dev_verified = developer.verified;

        Mod::create(json, developer, pool).await?;
        ModVersion::create_from_json(json, dev_verified, pool).await?;
        Ok(())
    }

    pub async fn new_version(
        json: &ModJson,
        developer: FetchedDeveloper,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let result = sqlx::query!(
            "SELECT DISTINCT m.id FROM mods m
            INNER JOIN mod_versions mv ON mv.mod_id = m.id
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE m.id = $1 AND mvs.status = 'accepted'",
            json.id
        )
        .fetch_optional(&mut *pool)
        .await
        .or(Err(ApiError::DbError))?;
        if result.is_none() {
            return Err(ApiError::NotFound(format!(
                "Mod {} doesn't exist or isn't yet validated",
                &json.id
            )));
        }

        let latest = match sqlx::query!(
            "SELECT mv.version, mv.id FROM mod_versions mv
            INNER JOIN mods m ON mv.mod_id = m.id
            WHERE m.id = $1
            ORDER BY mv.id DESC LIMIT 1",
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

        let version = semver::Version::parse(latest.version.trim_start_matches('v')).unwrap();
        let new_version = match semver::Version::parse(json.version.trim_start_matches('v')) {
            Ok(v) => v,
            Err(_) => {
                return Err(ApiError::BadRequest(format!(
                    "Invalid semver {}",
                    json.version
                )))
            }
        };
        if new_version.le(&version) {
            return Err(ApiError::BadRequest(format!(
                "mod.json version {} is smaller / equal to latest mod version {}",
                json.version, latest.version
            )));
        }
        ModVersion::create_from_json(json, developer.verified, pool).await?;

        Mod::update_existing_with_json(json, developer.verified, pool).await?;

        Ok(())
    }

    /**
     * At the moment this only sets the mod to featured, can be expanded with more stuff
     */
    pub async fn update_mod(
        id: &str,
        featured: bool,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if (match sqlx::query!("SELECT id FROM mods WHERE id = $1", id)
            .fetch_optional(&mut *pool)
            .await
        {
            Ok(e) => e,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        })
        .is_none()
        {
            return Err(ApiError::NotFound(format!("Mod {} doesn't exist", id)));
        }

        let result = match sqlx::query!("UPDATE mods SET featured = $1 WHERE id = $2", featured, id)
            .execute(&mut *pool)
            .await
        {
            Ok(e) => e,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        if result.rows_affected() == 0 {
            return Err(ApiError::InternalError);
        }

        Ok(())
    }

    pub async fn get_logo_for_mod(
        id: &str,
        pool: &mut PgConnection,
    ) -> Result<Option<Vec<u8>>, ApiError> {
        struct QueryResult {
            image: Option<Vec<u8>>
        }
        match sqlx::query_as!(
            QueryResult,
            "SELECT m.image 
            FROM mods m
            INNER JOIN mod_versions mv ON mv.mod_id = m.id 
            INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
            WHERE m.id = $1",
            id
        )
        .fetch_optional(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(Some(r)) => Ok(r.image),
            Ok(None) => Ok(None)
        }
    }

    async fn create(
        json: &ModJson,
        developer: FetchedDeveloper,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let updated: bool = match Mod::check_for_existing(&json.id, pool).await? {
            CheckExistingResult::Exists => {
                return Err(ApiError::BadRequest(format!(
                    "Mod {} already exists, consider creating a new version",
                    json.id
                )))
            },
            CheckExistingResult::ExistsNotValidated => {
                return Err(ApiError::BadRequest(format!(
                    "Mod {} already exists, but is not yet validated",
                    json.id
                )))
            },
            CheckExistingResult::ExistsWithRejected => {
                if !Developer::has_access_to_mod(developer.id, &json.id, pool).await? {
                    return Err(ApiError::Forbidden);
                }
                Mod::update_existing_with_json(json, developer.verified, pool).await?;

                if let Err(e) = sqlx::query!(
                    "delete from mod_versions mv
                    using mod_version_statuses mvs
                    where mv.id = mvs.mod_version_id and mv.mod_id = $1 and mvs.status = 'rejected'",
                    &json.id
                ).execute(&mut *pool).await {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                }

                true
            },
            CheckExistingResult::NotExists => false 
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
        if json.repository.is_some() {
            separated.push_bind(json.repository.as_ref().unwrap());
        }
        if json.changelog.is_some() {
            separated.push_bind(&json.changelog);
        }
        if json.about.is_some() {
            separated.push_bind(&json.about);
        }
        separated.push_bind(&json.id);
        separated.push_bind(&json.logo);
        separated.push_unseparated(")");

        if let Err(e) = query_builder.build().execute(&mut *pool).await {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Mod::assign_owner(&json.id, developer.id, pool).await?;

        Ok(())
    }

    async fn check_for_existing(id: &str, pool: &mut PgConnection) -> Result<CheckExistingResult, ApiError> {
        let res = match sqlx::query!("SELECT id FROM mods WHERE id = $1", id)
            .fetch_optional(&mut *pool)
            .await {
            Ok(e) => e,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };
        match res {
            None => Ok(CheckExistingResult::NotExists),
            Some(_) => {
                struct Counts {
                    not_rejected: i64,
                    rejected: i64,
                    validated: i64,
                }

                let counts = match sqlx::query!(
                    "select 
                    count(1) filter (where mvs.status = ANY(array['accepted', 'pending']::mod_version_status[])) as not_rejected,
                    count(1) filter (where mvs.status = 'rejected') as rejected,
                    count(1) filter (where mvs.status = 'accepted') as validated
                    from mod_versions mv
                    inner join mod_version_statuses mvs on mvs.mod_version_id = mv.id
                    where mv.mod_id = $1",
                    id
                )
                .fetch_one(&mut *pool)
                .await
                {
                    Ok(e) => {
                        Counts {
                            not_rejected: e.not_rejected.unwrap_or(0),
                            rejected: e.rejected.unwrap_or(0),
                            validated: e.validated.unwrap_or(0),
                        } 
                    },
                    Err(e) => {
                        log::error!("{}", e);
                        return Err(ApiError::DbError);
                    }
                };

                if counts.validated > 0 {
                    return Ok(CheckExistingResult::Exists);
                }
                if counts.validated == 0 && counts.not_rejected > 0 {
                    return Ok(CheckExistingResult::ExistsNotValidated);
                } 
                if counts.rejected > 0 {
                    return Ok(CheckExistingResult::ExistsWithRejected)
                }

                // This probably shouldn't ever happen. If it does I'm jumping off a cliff.
                Ok(CheckExistingResult::NotExists)
            }
        }
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
            ).execute(&mut *pool).await
            {
                Err(e) => {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                },
                Ok(r) => {
                    if r.rows_affected() == 0 {
                        log::error!("Couldn't update timestamp on mod. No rows affected.");
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn calculate_cached_downloads(
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if let Err(e) = sqlx::query!(
            "UPDATE mods m SET download_count = (
                SELECT COUNT(DISTINCT md.ip) FROM mod_downloads md
                INNER JOIN mod_versions mv ON md.mod_version_id = mv.id
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                WHERE mv.mod_id = m.id AND mvs.status = 'accepted'
            ), last_download_cache_refresh = now()
            WHERE m.id = $1", mod_id
        ).execute(&mut *pool).await {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(())
    }

    pub async fn assign_owner(
        mod_id: &str,
        dev_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let existing = sqlx::query!(
            "SELECT md.developer_id, md.is_owner FROM mods_developers md
            INNER JOIN mods m ON md.mod_id = m.id
            WHERE m.id = $1",
            mod_id
        )
        .fetch_all(&mut *pool)
        .await;

        let existing = match existing {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(e) => e,
        };

        if !existing.is_empty() {
            let res = sqlx::query!(
                "UPDATE mods_developers SET is_owner = false
                WHERE mod_id = $1",
                mod_id
            )
            .execute(&mut *pool)
            .await;

            if let Err(e) = res {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        }

        for record in existing {
            // we found our dev inside the existing list
            if record.developer_id == dev_id {
                if let Err(e) = sqlx::query!(
                    "UPDATE mods_developers SET is_owner = true
                    WHERE mod_id = $1 AND developer_id = $2",
                    mod_id,
                    dev_id
                )
                .execute(&mut *pool)
                .await
                {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                }
                return Ok(());
            }
        }

        if let Err(e) = sqlx::query!(
            "INSERT INTO mods_developers (mod_id, developer_id, is_owner) VALUES
            ($1, $2, true)",
            mod_id,
            dev_id
        )
        .execute(&mut *pool)
        .await
        {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(())
    }

    pub async fn assign_dev(
        mod_id: &str,
        dev_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let existing = match sqlx::query!(
            "SELECT md.developer_id, md.is_owner FROM mods_developers md
            INNER JOIN mods m ON md.mod_id = m.id
            WHERE m.id = $1",
            mod_id
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(e) => e,
            Err(err) => {
                log::error!("{}", err);
                return Err(ApiError::DbError);
            }
        };

        if existing.iter().any(|x| x.developer_id == dev_id) {
            return Err(ApiError::BadRequest(format!(
                "This developer already exists on mod {}",
                mod_id
            )));
        }

        match sqlx::query!(
            "INSERT INTO mods_developers (mod_id, developer_id)
            VALUES ($1, $2)",
            mod_id,
            dev_id
        )
        .execute(&mut *pool)
        .await
        {
            Err(err) => {
                log::error!("{}", err);
                Err(ApiError::DbError)
            }
            Ok(_) => Ok(()),
        }
    }

    pub async fn unassign_dev(
        mod_id: &str,
        dev_id: i32,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let existing = match sqlx::query!(
            "SELECT md.developer_id, md.is_owner FROM mods_developers md
            INNER JOIN mods m ON md.mod_id = m.id
            WHERE m.id = $1",
            mod_id
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(e) => e,
            Err(err) => {
                log::error!("{}", err);
                return Err(ApiError::DbError);
            }
        };

        let found = match existing.iter().find(|x| x.developer_id == dev_id) {
            None => {
                return Err(ApiError::NotFound(
                    "Developer is not assigned to mod".to_string(),
                ))
            }
            Some(f) => f,
        };

        if found.is_owner {
            return Err(ApiError::BadRequest(
                "Cannot unassign the owner developer for the mod".to_string(),
            ));
        }

        match sqlx::query!(
            "DELETE FROM mods_developers
            WHERE mod_id = $1 AND developer_id = $2",
            mod_id,
            dev_id
        )
        .execute(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(_) => Ok(()),
        }
    }

    pub async fn get_updates(
        ids: &[String],
        platforms: VerPlatform,
        geode: &semver::Version,
        gd: GDVersionEnum,
        pool: &mut PgConnection,
    ) -> Result<Vec<ModUpdate>, ApiError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT 
                q.id, 
                q.inner_version as version, 
                q.mod_version_id
            FROM (
                SELECT m.id, 
                    mv.id as mod_version_id,
                    mv.version as inner_version,
                    row_number() over (partition by m.id order by mv.id desc) rn 
                FROM mods m
                INNER JOIN mod_versions mv ON mv.mod_id = m.id 
                INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
                INNER JOIN mod_gd_versions mgv ON mv.id = mgv.mod_id
                WHERE mvs.status = 'accepted' 
                    AND mgv.platform = "#
        );
        builder.push_bind(platforms);
        builder.push(" AND (mgv.gd = ");
        builder.push_bind(gd);
        builder.push(" OR mgv.gd = '*')");

        builder.push(" AND m.id = ANY(");
        builder.push_bind(ids);
        builder.push(") ");

        if geode.pre.contains("alpha") {
            builder.push(" AND mv.geode = ");
            builder.push_bind(geode.to_string());
        } else {
            let sql = " AND (SPLIT_PART(mv.geode, '.', 1) = ";
            builder.push(sql);
            builder.push_bind(geode.major.to_string());

            let sql = " AND SPLIT_PART(mv.geode, '-', 2) NOT LIKE 'alpha%' AND SPLIT_PART(mv.geode, '.', 2) <= ";
            builder.push(sql);
            builder.push_bind(geode.minor.to_string());

            // Match only higher betas (or no beta)
            if geode.pre.contains("beta") {
                let sql = " AND (SPLIT_PART(mv.geode, '-', 2) = ''
                    OR SPLIT_PART(mv.geode, '-', 2) <=";
                builder.push(sql);
                builder.push_bind(geode.pre.to_string());
                builder.push(")");
            }

            builder.push(")");
        }

        builder.push(") q where q.rn = 1");

        #[derive(sqlx::FromRow)]
        struct QueryResult {
            id: String,
            version: String,
            mod_version_id: i32,
        }

        let result = match builder
            .build_query_as::<QueryResult>()
            .fetch_all(&mut *pool)
            .await
        {
            Ok(e) => e,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

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
                replacement: None
            };
            ret.push(update);
        }

        Ok(ret)
    }

    pub async fn update_mod_image(id: &str, hash: &str, download_link: &str, pool: &mut PgConnection) -> Result<(), ApiError> {
        let mut cursor = download_geode_file(download_link).await?;
        let mut bytes: Vec<u8> = vec![];
        match cursor.read_to_end(&mut bytes) {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::FilesystemError);
            }
            Ok(b) => b,
        };

        let new_hash = sha256::digest(bytes);
        if new_hash != hash {
            return Err(ApiError::BadRequest(format!("Different hash detected: old: {}, new: {}", hash, new_hash)));
        }
        let mut archive = match zip::ZipArchive::new(cursor) {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::BadRequest(
                    "Couldn't unzip .geode file".to_string(),
                ));
            }
            Ok(a) => a,
        };

        let mut image_file = match archive.by_name("logo.png") {
            Err(e) => {
                // In this case maybe the mod doesn't have a logo
                log::error!("{}", e);
                return Ok(());
            },
            Ok(i) => i
        };

        let image = mod_json::validate_mod_logo(&mut image_file, true)?;

        match sqlx::query!(
            "UPDATE mods SET image = $1
            WHERE id = $2",
            image,
            id
        ).execute(&mut *pool)
        .await {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            },
            Ok(m) => {
                if m.rows_affected() == 0 {
                    log::error!("No rows affected by image update");
                    return Err(ApiError::DbError);
                }
            }
        }

        Ok(())
    }
}

pub async fn download_geode_file(url: &str) -> Result<Cursor<Bytes>, ApiError> {
    let size = get_download_size(url).await?;
    if size > 1_000_000_000 {
        return Err(ApiError::BadRequest(
            "File size is too large, max 100MB".to_string(),
        ));
    }
    let res = reqwest::get(url)
        .await
        .or(Err(ApiError::BadRequest(String::from("Invalid URL"))))?;
    let content = Cursor::new(res.bytes().await.or(Err(ApiError::FilesystemError))?);
    Ok(content)
}

async fn get_download_size(url: &str) -> Result<u64, ApiError> {
    let client = Client::new();

    let res = client
        .head(url)
        .send()
        .await
        .or(Err(ApiError::BadRequest(String::from("Invalid URL"))))?;

    match res.headers().get("content-length") {
        Some(s) => {
            if let Ok(s) = s.to_str() {
                if let Ok(s) = s.parse::<u64>() {
                    return Ok(s);
                }
            }
            Err(ApiError::BadRequest(
                "Couldn't extract download size from URL".to_string(),
            ))
        }
        None => Err(ApiError::BadRequest(
            "Couldn't extract download size from URL".to_string(),
        )),
    }
}