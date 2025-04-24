use crate::types::{api::ApiError, mod_json::ModJson, models::mod_entity::Mod};
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::PgConnection;

#[derive(sqlx::FromRow)]
struct ModRecordGetOne {
    id: String,
    repository: Option<String>,
    featured: bool,
    download_count: i32,
    #[sqlx(default)]
    about: Option<String>,
    #[sqlx(default)]
    changelog: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl ModRecordGetOne {
    pub fn into_mod(self) -> Mod {
        Mod {
            id: self.id,
            repository: self.repository,
            featured: self.featured,
            download_count: self.download_count,
            versions: Default::default(),
            tags: Default::default(),
            developers: Default::default(),
            created_at: self.created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            updated_at: self.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            about: self.about.clone(),
            changelog: self.changelog.clone(),
            links: None,
        }
    }
}

/// Fetches information for a mod, without versions or other added info.
///
/// The second parameter decides if about.md and changelog.md are fetched from the database. Those are pretty big files, so only fetch them if needed.
pub async fn get_one(
    id: &str,
    include_md: bool,
    conn: &mut PgConnection,
) -> Result<Option<Mod>, ApiError> {
    if include_md {
        sqlx::query_as!(
            ModRecordGetOne,
            "SELECT
                m.id, m.repository, m.about, m.changelog, m.featured,
                m.download_count, m.created_at, m.updated_at
            FROM mods m
            WHERE id = $1",
            id
        )
        .fetch_optional(conn)
        .await
        .inspect_err(|e| log::error!("Failed to fetch mod {}: {}", id, e))
        .or(Err(ApiError::DbError))
        .map(|x| x.map(|x| x.into_mod()))
    } else {
        sqlx::query_as!(
            ModRecordGetOne,
            "SELECT
            m.id, m.repository, NULL as about, NULL as changelog, m.featured,
            m.download_count, m.created_at, m.updated_at
        FROM mods m
        WHERE id = $1",
            id
        )
        .fetch_optional(conn)
        .await
        .inspect_err(|e| log::error!("Failed to fetch mod {}: {}", id, e))
        .or(Err(ApiError::DbError))
        .map(|x| x.map(|x| x.into_mod()))
    }
}

/// Does NOT check if the target mod exists
pub async fn create(json: &ModJson, conn: &mut PgConnection) -> Result<Mod, ApiError> {
    sqlx::query_as!(
        ModRecordGetOne,
        "INSERT INTO mods (
            id,
            repository,
            changelog,
            about,
            image
        ) VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id, repository, about,
            changelog, featured,
            download_count, created_at,
            updated_at",
        &json.id,
        json.repository,
        json.changelog,
        json.about,
        &vec![]
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| log::error!("Failed to created mod {}: {}", &json.id, e))
    .or(Err(ApiError::DbError))
    .map(|x| x.into_mod())
}

pub async fn assign_owner(
    id: &str,
    developer_id: i32,
    conn: &mut PgConnection,
) -> Result<(), ApiError> {
    assign_developer(id, developer_id, true, conn).await
}

pub async fn assign_developer(
    id: &str,
    developer_id: i32,
    owner: bool,
    conn: &mut PgConnection,
) -> Result<(), ApiError> {
    sqlx::query!(
        "INSERT INTO mods_developers (mod_id, developer_id, is_owner)
        VALUES ($1, $2, $3)",
        id,
        developer_id,
        owner
    )
    .execute(conn)
    .await
    .inspect_err(|x| {
        log::error!(
            "Couldn't assign developer {} on mod {} (owner {}): {}",
            developer_id,
            id,
            owner,
            x
        )
    })
    .or(Err(ApiError::DbError))?;

    Ok(())
}

pub async fn is_featured(id: &str, conn: &mut PgConnection) -> Result<bool, ApiError> {
    Ok(sqlx::query!("SELECT featured FROM mods WHERE id = $1", id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| {
            log::error!("Failed to check if mod {} exists: {}", id, e);
            ApiError::DbError
        })?
        .map(|row| row.featured)
        .unwrap_or(false))
}

pub async fn exists(id: &str, conn: &mut PgConnection) -> Result<bool, ApiError> {
    Ok(sqlx::query!("SELECT id FROM mods WHERE id = $1", id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| {
            log::error!("Failed to check if mod {} exists: {}", id, e);
            ApiError::DbError
        })?
        .is_some())
}

pub async fn get_logo(id: &str, conn: &mut PgConnection) -> Result<Option<Vec<u8>>, ApiError> {
    struct QueryResult {
        image: Option<Vec<u8>>,
    }

    let vec = sqlx::query_as!(
        QueryResult,
        "SELECT
            m.image
        FROM mods m
        INNER JOIN mod_versions mv ON mv.mod_id = m.id
        INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
        WHERE m.id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch mod logo for {}: {}", id, e);
        ApiError::DbError
    })?
    .and_then(|optional| optional.image);

    // Empty vec means no image
    if vec.as_ref().is_some_and(|v| v.is_empty()) {
        Ok(None)
    } else {
        Ok(vec)
    }
}

pub async fn increment_downloads(id: &str, conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE mods
        SET download_count = download_count + 1
        WHERE id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to increment downloads for mod {}: {}", id, e);
        ApiError::DbError
    })?;

    Ok(())
}

pub async fn update_with_json(
    mut the_mod: Mod,
    json: &ModJson,
    conn: &mut PgConnection,
) -> Result<Mod, ApiError> {
    sqlx::query!(
        "UPDATE mods
        SET repository = $1,
        about = $2,
        changelog = $3,
        image = $4,
        updated_at = NOW()",
        json.repository,
        json.about,
        json.changelog,
        json.logo
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("Failed to update mod: {}", e))
    .or(Err(ApiError::DbError))?;

    the_mod.repository = json.repository.clone();
    the_mod.about = json.about.clone();
    the_mod.changelog = json.changelog.clone();

    Ok(the_mod)
}

pub async fn update_with_json_moved(
    mut the_mod: Mod,
    json: ModJson,
    conn: &mut PgConnection,
) -> Result<Mod, ApiError> {
    sqlx::query!(
        "UPDATE mods
        SET repository = $1,
        about = $2,
        changelog = $3,
        image = $4,
        updated_at = NOW()",
        json.repository,
        json.about,
        json.changelog,
        json.logo
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("Failed to update mod: {}", e))
    .or(Err(ApiError::DbError))?;

    the_mod.repository = json.repository;
    the_mod.about = json.about;
    the_mod.changelog = json.changelog;

    Ok(the_mod)
}
