use crate::types::{
    api::ApiError,
    mod_json::ModJson,
    models::{
        developer::Developer, mod_version::ModVersion, mod_version_status::ModVersionStatusEnum,
    },
};
use chrono::{DateTime, SecondsFormat, Utc};
use semver::Version;
use sqlx::PgConnection;

use super::mod_version_statuses;

#[derive(sqlx::FromRow)]
struct ModVersionRow {
    id: i32,
    name: String,
    description: Option<String>,
    version: String,
    download_link: String,
    download_count: i32,
    hash: String,
    geode: String,
    early_load: bool,
    api: bool,
    mod_id: String,
    status: ModVersionStatusEnum,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    info: Option<String>,
}

impl ModVersionRow {
    pub fn into_mod_version(self) -> ModVersion {
        ModVersion {
            id: self.id,
            name: self.name,
            description: self.description,
            version: self.version,
            download_link: self.download_link,
            hash: self.hash,
            geode: self.geode,
            early_load: self.early_load,
            download_count: self.download_count,
            api: self.api,
            mod_id: self.mod_id,
            status: self.status,
            gd: Default::default(),
            developers: None,
            tags: None,
            dependencies: None,
            incompatibilities: None,
            info: self.info,
            direct_download_link: None,
            created_at: self
                .created_at
                .map(|x| x.to_rfc3339_opts(SecondsFormat::Secs, true)),
            updated_at: self
                .updated_at
                .map(|x| x.to_rfc3339_opts(SecondsFormat::Secs, true)),
        }
    }
}

pub async fn get_by_version_str(
    mod_id: &str,
    version: &str,
    conn: &mut PgConnection,
) -> Result<Option<ModVersion>, ApiError> {
    sqlx::query_as!(
        ModVersionRow,
        r#"SELECT
            mv.id, mv.name, mv.description, mv.version,
            mv.download_link, mv.download_count, mv.hash,
            format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as "geode!: _",
            mv.early_load, mv.api, mv.mod_id,
            mv.created_at, mv.updated_at,
            mvs.status as "status: _", mvs.info
        FROM mod_versions mv
        INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
        WHERE mv.mod_id = $1
        AND mv.version = $2"#,
        mod_id,
        version
    )
    .fetch_optional(conn)
    .await
    .inspect_err(|e| log::error!("{}", e))
    .or(Err(ApiError::DbError))
    .map(|opt| opt.map(|x| x.into_mod_version()))
}

pub async fn get_for_mod(
    mod_id: &str,
    statuses: Option<&[ModVersionStatusEnum]>,
    conn: &mut PgConnection,
) -> Result<Vec<ModVersion>, ApiError> {
    sqlx::query_as!(
        ModVersionRow,
        r#"SELECT
            mv.id, mv.name, mv.description, mv.version,
            mv.download_link, mv.download_count, mv.hash,
            format_semver(mv.geode_major, mv.geode_minor, mv.geode_patch, mv.geode_meta) as "geode!: _",
            mv.early_load, mv.api, mv.mod_id,
            mv.created_at, mv.updated_at,
            mvs.status as "status: _", mvs.info
        FROM mod_versions mv
        INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
        WHERE mv.mod_id = $1
        AND ($2::mod_version_status[] IS NULL OR mvs.status = ANY($2))
        ORDER BY mv.id DESC"#,
        mod_id,
        statuses as Option<&[ModVersionStatusEnum]>
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| log::error!("{}", e))
    .or(Err(ApiError::DbError))
    .map(|opt: Vec<ModVersionRow>| opt.into_iter().map(|x| x.into_mod_version()).collect())
}

pub async fn increment_downloads(id: i32, conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE mod_versions
        SET download_count = download_count + 1
        WHERE id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!(
            "Failed to increment downloads for mod_version {}: {}",
            id,
            e
        );
        ApiError::DbError
    })?;

    Ok(())
}

pub async fn create_from_json(
    json: &ModJson,
    make_accepted: bool,
    conn: &mut PgConnection,
) -> Result<ModVersion, ApiError> {
    sqlx::query!("SET CONSTRAINTS mod_versions_status_id_fkey DEFERRED")
        .execute(&mut *conn)
        .await
        .inspect_err(|e| log::error!("Failed to update constraint: {}", e))
        .or(Err(ApiError::DbError))?;

    let geode = Version::parse(&json.geode).or(Err(ApiError::BadRequest(
        "Invalid geode version in mod.json".into(),
    )))?;

    let meta = if geode.pre.is_empty() {
        None
    } else {
        Some(geode.pre.to_string())
    };

    let row = sqlx::query!(
        "INSERT INTO mod_versions
        (name, version, description, download_link,
        hash, geode_major, geode_minor, geode_patch, geode_meta,
        early_load, api, mod_id, status_id,
        created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 0,
        NOW(), NOW())
        RETURNING
            id, name, version, description,
            download_link, hash,
            early_load, api, mod_id,
            created_at, updated_at",
        json.name,
        json.version,
        json.description,
        json.download_url,
        json.hash,
        i32::try_from(geode.major).unwrap_or_default(),
        i32::try_from(geode.minor).unwrap_or_default(),
        i32::try_from(geode.patch).unwrap_or_default(),
        meta,
        json.early_load,
        json.api.is_some(),
        json.id
    )
    .fetch_one(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to insert mod_version: {}", e))
    .or(Err(ApiError::DbError))?;

    let id = row.id;

    let status = match make_accepted {
        true => ModVersionStatusEnum::Accepted,
        false => ModVersionStatusEnum::Pending,
    };

    let status_id = mod_version_statuses::create(id, status, None, conn).await?;
    sqlx::query!(
        "UPDATE mod_versions SET status_id = $1 WHERE id = $2",
        status_id,
        id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to set status: {}", e))
    .or(Err(ApiError::DbError))?;

    sqlx::query!("SET CONSTRAINTS mod_versions_status_id_fkey IMMEDIATE")
        .execute(&mut *conn)
        .await
        .inspect_err(|e| log::error!("Failed to update constraint: {}", e))
        .or(Err(ApiError::DbError))?;

    Ok(ModVersion {
        id,
        name: row.name,
        description: row.description,
        version: row.version,
        download_link: row.download_link,
        hash: row.hash,
        geode: geode.to_string(),
        download_count: 0,
        early_load: row.early_load,
        api: row.api,
        mod_id: row.mod_id,
        gd: Default::default(),
        status,
        dependencies: Default::default(),
        incompatibilities: Default::default(),
        developers: Default::default(),
        tags: Default::default(),
        created_at: row
            .created_at
            .map(|i| i.to_rfc3339_opts(SecondsFormat::Secs, true)),
        updated_at: row
            .updated_at
            .map(|i| i.to_rfc3339_opts(SecondsFormat::Secs, true)),
        info: None,
        direct_download_link: None,
    })
}

pub async fn update_pending_version(
    version_id: i32,
    json: &ModJson,
    make_accepted: bool,
    conn: &mut PgConnection,
) -> Result<ModVersion, ApiError> {
    let geode = Version::parse(&json.geode).or(Err(ApiError::BadRequest(
        "Invalid geode version in mod.json".into(),
    )))?;

    let meta = if geode.pre.is_empty() {
        None
    } else {
        Some(geode.pre.to_string())
    };

    let row = sqlx::query!(
        "UPDATE mod_versions mv
            SET name = $1,
            version = $2,
            download_link = $3,
            hash = $4,
            geode_major = $5,
            geode_minor = $6,
            geode_patch = $7,
            geode_meta = $8,
            early_load = $9,
            api = $10,
            description = $11,
            updated_at = NOW()
        FROM mod_version_statuses mvs
        WHERE mv.status_id = mvs.id
        AND mvs.status = 'pending'
        AND mv.id = $12
        RETURNING mv.id,
            name,
            version,
            download_link,
            download_count,
            hash,
            early_load,
            api,
            status_id,
            description,
            mod_id,
            mv.created_at,
            mv.updated_at",
        &json.name,
        &json.version,
        &json.download_url,
        &json.hash,
        i32::try_from(geode.major).unwrap_or_default(),
        i32::try_from(geode.minor).unwrap_or_default(),
        i32::try_from(geode.patch).unwrap_or_default(),
        meta,
        &json.early_load,
        &json.api.is_some(),
        json.description.clone().unwrap_or_default(),
        version_id
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|err| {
        log::error!(
            "Failed to update pending version {}-{}: {}",
            json.id,
            json.version,
            err
        );
        ApiError::DbError
    })?;

    if make_accepted {
        sqlx::query!(
            "UPDATE mod_version_statuses
            SET status = 'accepted'
            WHERE id = $1",
            row.status_id
        )
        .execute(&mut *conn)
        .await
        .inspect_err(|e| log::error!("Failed to update tag for mod: {}", e))
        .or(Err(ApiError::DbError))?;
    }

    Ok(ModVersion {
        id: version_id,
        name: row.name,
        description: row.description,
        version: row.version,
        download_link: row.download_link,
        hash: row.hash,
        geode: geode.to_string(),
        download_count: row.download_count,
        early_load: row.early_load,
        api: row.api,
        mod_id: row.mod_id,
        gd: Default::default(),
        status: match make_accepted {
            true => ModVersionStatusEnum::Accepted,
            false => ModVersionStatusEnum::Pending,
        },
        dependencies: Default::default(),
        incompatibilities: Default::default(),
        developers: Default::default(),
        tags: Default::default(),
        created_at: row
            .created_at
            .map(|i| i.to_rfc3339_opts(SecondsFormat::Secs, true)),
        updated_at: row
            .updated_at
            .map(|i| i.to_rfc3339_opts(SecondsFormat::Secs, true)),
        info: None,
        direct_download_link: None,
    })
}

pub async fn update_version_status(
    mut version: ModVersion,
    status: ModVersionStatusEnum,
    info: Option<&str>,
    updated_by: &Developer,
    conn: &mut PgConnection,
) -> Result<ModVersion, ApiError> {
    if version.status == status {
        return Ok(version);
    }

    sqlx::query!(
        "UPDATE mod_version_statuses
        SET status = $1,
        admin_id = $2,
        info = $3,
        updated_at = NOW()
        WHERE mod_version_id = $4",
        status as ModVersionStatusEnum,
        updated_by.id,
        info,
        version.id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| log::error!("{}", e))
    .or(Err(ApiError::DbError))?;

    version.status = status;

    Ok(version)
}

pub async fn touch_updated_at(id: i32, conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE mod_versions
        SET updated_at = NOW()
        WHERE id = $1",
        id
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("Failed to touch updated_at for mod version {}: {}", id, e))
    .or(Err(ApiError::DbError))?;

    Ok(())
}
