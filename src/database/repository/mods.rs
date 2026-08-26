use crate::{
    database::DatabaseError,
    types::{
        mod_json::ModJson,
        models::{mod_entity::Mod, mod_status::ModStatusEnum}
    },
};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum ModLogo {
    Data(Vec<u8>),
    Url(String),
}

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
    status: ModStatusEnum,
    #[sqlx(default)]
    status_info: Option<String>,
}

impl ModRecordGetOne {
    pub fn into_mod(self) -> Mod {
        Mod {
            id: self.id,
            repository: self.repository,
            featured: self.featured,
            download_count: self.download_count.into(),
            versions: Default::default(),
            tags: Default::default(),
            developers: Default::default(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            about: self.about.clone(),
            changelog: self.changelog.clone(),
            links: None,
            status: self.status,
            status_info: self.status_info,
        }
    }
}

/// Fetches information for a mod, without versions or other added info.
///
/// The second parameter decides if about.md and changelog.md are fetched from the database. Those are pretty big files, so only fetch them if needed.
#[tracing::instrument(skip_all, fields(mod_id = %id))]
pub async fn get_one(
    id: &str,
    include_md: bool,
    conn: &mut PgConnection,
) -> Result<Option<Mod>, DatabaseError> {
    if include_md {
        sqlx::query_as!(
            ModRecordGetOne,
            r#"SELECT
                m.id, m.repository, m.about, m.changelog, m.featured,
                m.download_count, m.created_at, m.updated_at,
                m.status AS "status: _", m.status_info
            FROM mods m
            WHERE m.id = $1"#,
            id
        )
        .fetch_optional(conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))
        .map_err(|e| e.into())
        .map(|x| x.map(|x| x.into_mod()))
    } else {
        sqlx::query_as!(
            ModRecordGetOne,
            r#"SELECT
            m.id, m.repository, NULL as about, NULL as changelog, m.featured,
            m.download_count, m.created_at, m.updated_at,
            m.status AS "status: _", NULL AS status_info
        FROM mods m
        WHERE m.id = $1"#,
            id
        )
        .fetch_optional(conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))
        .map_err(|e| e.into())
        .map(|x| x.map(|x| x.into_mod()))
    }
}

/// Does NOT check if the target mod exists
#[tracing::instrument(skip_all, fields(mod_id = %json.id))]
pub async fn create(json: &ModJson, conn: &mut PgConnection) -> Result<Mod, DatabaseError> {
    sqlx::query_as!(
        ModRecordGetOne,
        r#"INSERT INTO mods (
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
            updated_at, status as "status: _",
            status_info"#,
        &json.id,
        json.repository,
        json.changelog,
        json.about,
        &vec![]
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map_err(|e| e.into())
    .map(|x| x.into_mod())
}

#[tracing::instrument(skip_all, fields(mod_id = %id, developer_id = %developer_id))]
pub async fn assign_owner(
    id: &str,
    developer_id: i32,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    assign_developer(id, developer_id, true, conn).await
}

#[tracing::instrument(skip_all, fields(mod_id = %id, developer_id = %developer_id, owner = %owner))]
pub async fn assign_developer(
    id: &str,
    developer_id: i32,
    owner: bool,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query!(
        "INSERT INTO mods_developers (mod_id, developer_id, is_owner)
        VALUES ($1, $2, $3)",
        id,
        developer_id,
        owner
    )
    .execute(conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map(|_| ())
    .map_err(|e| e.into())
}

#[tracing::instrument(skip_all, fields(mod_id = %id, developer_id = %developer_id))]
pub async fn unassign_developer(
    id: &str,
    developer_id: i32,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM mods_developers
        WHERE mod_id = $1
        AND developer_id = $2",
        id,
        developer_id
    )
    .execute(conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map(|_| ())
    .map_err(|e| e.into())
}

#[tracing::instrument(skip_all, fields(mod_id = %id))]
pub async fn is_featured(id: &str, conn: &mut PgConnection) -> Result<bool, DatabaseError> {
    Ok(sqlx::query!("SELECT featured FROM mods WHERE id = $1", id)
        .fetch_optional(&mut *conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?
        .map(|row| row.featured)
        .unwrap_or(false))
}

#[tracing::instrument(skip_all, fields(mod_id = %id))]
pub async fn exists(id: &str, conn: &mut PgConnection) -> Result<bool, DatabaseError> {
    Ok(sqlx::query!("SELECT id FROM mods WHERE id = $1", id)
        .fetch_optional(&mut *conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?
        .is_some())
}

/// Checks if multiple ids exist in the database.
///
/// Returns a tuple with (existing ids, missing ids).
#[tracing::instrument(skip_all, fields(mod_ids = ?ids))]
pub async fn exists_multiple(
    ids: &[String],
    conn: &mut PgConnection,
) -> Result<(Vec<String>, Vec<String>), DatabaseError> {
    let mods: HashSet<String> = sqlx::query!("SELECT id FROM mods WHERE id = ANY($1)", ids)
        .fetch_all(&mut *conn)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?
        .into_iter()
        .map(|x| x.id)
        .collect();

    let (mut existing, mut missing): (Vec<String>, Vec<String>) =
        (Vec::with_capacity(ids.len()), vec![]);

    for id in ids {
        if mods.contains(id) {
            existing.push(id.clone());
        } else {
            missing.push(id.clone());
        }
    }

    Ok((existing, missing))
}

#[tracing::instrument(skip_all, fields(mod_id = %id))]
pub async fn get_logo(id: &str, conn: &mut PgConnection) -> Result<Option<ModLogo>, DatabaseError> {
    struct QueryResult {
        image: Option<Vec<u8>>,
        image_url: Option<String>,
    }

    let logo = sqlx::query_as!(
        QueryResult,
        "SELECT
            m.image,
            m.image_url
        FROM mods m
        INNER JOIN mod_versions mv ON mv.mod_id = m.id
        INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
        WHERE m.id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?
    .and_then(|r| {
        if let Some(url) = r.image_url {
            Some(ModLogo::Url(url))
        } else if let Some(data) = r.image
            && !data.is_empty()
        {
            Some(ModLogo::Data(data))
        } else {
            None
        }
    });

    Ok(logo)
}

#[tracing::instrument(skip_all, fields(mod_id = %id))]
pub async fn increment_downloads(id: &str, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!(
        "UPDATE mods
        SET download_count = download_count + 1
        WHERE id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}

#[tracing::instrument(skip_all, fields(mod_id = %the_mod.id))]
pub async fn update_with_json_moved(
    mut the_mod: Mod,
    json: ModJson,
    conn: &mut PgConnection,
) -> Result<Mod, DatabaseError> {
    sqlx::query!(
        "UPDATE mods
        SET repository = $1,
        about = $2,
        changelog = $3,
        image = $4,
        image_url = NULL,
        updated_at = NOW()
        WHERE id = $5",
        json.repository,
        json.about,
        json.changelog,
        json.logo,
        the_mod.id
    )
    .execute(conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    the_mod.repository = json.repository;
    the_mod.about = json.about;
    the_mod.changelog = json.changelog;

    Ok(the_mod)
}

/// Updates the logo URL in the database and sets the logo data to null.
#[tracing::instrument(skip_all, fields(id = %id, url = %url))]
pub async fn update_mod_logo_url(
    id: &str,
    url: &str,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query!(
        "UPDATE mods SET image = NULL, image_url = $1 WHERE id = $2",
        url,
        id
    )
    .execute(conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}

/// Used when first version goes from pending to accepted.
/// Makes it so versions that stay a lot in pending appear at the top of the newly created lists
#[tracing::instrument(skip_all, fields(mod_id = %id))]
pub async fn touch_created_at(id: &str, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!(
        "UPDATE mods
        SET created_at = NOW()
        WHERE id = $1",
        id
    )
    .execute(conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(())
}

#[tracing::instrument(skip_all, fields(mod_id = %id))]
pub async fn has_status(
    id: &str,
    status: ModStatusEnum,
    pool: &mut PgConnection,
) -> Result<bool, DatabaseError> {
    sqlx::query_scalar!(r#"SELECT EXISTS(
            SELECT 1 FROM mods m
            WHERE m.id = $1 AND m.status = $2
        ) AS "exists!""#,
        id, status as ModStatusEnum
    )
    .fetch_one(&mut *pool)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map_err(|e| e.into())
}

#[tracing::instrument(skip_all, fields(mod_id = %id))]
pub async fn is_status_locked(
    id: &str,
    pool: &mut PgConnection,
) -> Result<bool, DatabaseError> {
    sqlx::query_scalar!(r#"SELECT EXISTS(
            SELECT 1 FROM mods m
            WHERE m.id = $1 AND m.status_locked = TRUE
        ) AS "exists!""#,
        id
    )
    .fetch_one(&mut *pool)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))
    .map_err(|e| e.into())
}