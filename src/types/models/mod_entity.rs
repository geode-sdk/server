use super::{
    dependency::ResponseDependency,
    developer::ModDeveloper,
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
        models::{mod_version::ModVersion, mod_version_status::ModVersionStatusEnum},
    },
};
use chrono::SecondsFormat;
use semver::Version;
use serde::Serialize;
use sqlx::{
    types::chrono::{DateTime, Utc},
    PgConnection, Postgres, QueryBuilder,
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
        let records: Vec<ModRecordGetOne> = sqlx::query_as!(
            ModRecordGetOne,
            r#"SELECT
                m.id, m.repository, m.about, m.changelog, m.featured, m.download_count as mod_download_count, m.created_at, m.updated_at,
                mv.id as version_id, mv.name, mv.description, mv.version, mv.download_link, mv.download_count as mod_version_download_count,
                mv.created_at as mod_version_created_at, mv.updated_at as mod_version_updated_at,
                mv.hash, mv.geode, mv.early_load, mv.api, mv.mod_id, mvs.status as "status: _", mvs.info
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
                    AND mgv.platform = "#,
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
                replacement: None,
            };
            ret.push(update);
        }

        Ok(ret)
    }
}
