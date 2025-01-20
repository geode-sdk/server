use crate::types::api::{ApiError, PaginatedData};
use crate::types::models::developer::{ModDeveloper, Developer};
use futures::TryFutureExt;
use sqlx::{PgConnection, Postgres, QueryBuilder};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub async fn index(
    query: Option<&String>,
    page: i64,
    per_page: i64,
    conn: &mut PgConnection,
) -> Result<PaginatedData<Developer>, ApiError> {
    let limit = per_page;
    let offset = (page - 1) * per_page;

    let display_name_query = query.map(|str| format!("%{}%", str));

    let result = sqlx::query_as!(
        Developer,
        "SELECT
            id,
            username,
            display_name,
            verified,
            admin
        FROM developers
        WHERE (
            ($1 = '' OR username = $1)
            OR ($2 = '' OR display_name ILIKE $2)
        )
        GROUP BY id
        LIMIT $3
        OFFSET $4",
        query,
        display_name_query,
        limit,
        offset
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch developers: {}", e);
        ApiError::DbError
    })?;

    let count = index_count(query, &mut *conn).await?;

    Ok(PaginatedData {
        data: result,
        count,
    })
}

pub async fn index_count(query: Option<&String>, conn: &mut PgConnection) -> Result<i64, ApiError> {
    let display_name_query = query.map(|str| format!("%{}%", str));

    Ok(sqlx::query!(
        "SELECT COUNT(id)
        FROM developers
        WHERE (
            ($1 = '' OR username = $1)
            OR ($2 = '' OR display_name ILIKE $2)
        )",
        query,
        display_name_query
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch developer count: {}", e);
        ApiError::DbError
    })?
    .count
    .unwrap_or(0))
}

pub async fn fetch_or_insert_github(
    github_id: i64,
    username: &str,
    conn: &mut PgConnection,
) -> Result<Developer, ApiError> {
    match sqlx::query_as!(
        Developer,
        "SELECT
            id,
            username,
            display_name,
            verified,
            admin
        FROM developers
        WHERE github_user_id = $1",
        github_id
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch developer for GitHub id: {}", e);
        ApiError::DbError
    })? {
        Some(dev) => Ok(dev),
        None => Ok(insert_github(github_id, username, conn).await?),
    }
}

async fn insert_github(
    github_id: i64,
    username: &str,
    conn: &mut PgConnection,
) -> Result<Developer, ApiError> {
    Ok(sqlx::query_as!(
        Developer,
        "INSERT INTO developers(username, display_name, github_user_id)
        VALUES ($1, $1, $2)
        RETURNING
            id,
            username,
            display_name,
            verified,
            admin",
        username,
        github_id
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to insert developer: {}", e);
        ApiError::DbError
    })?)
}

pub async fn get_one(
    id: i32,
    conn: &mut PgConnection,
) -> Result<Option<Developer>, ApiError> {
    Ok(sqlx::query_as!(
        Developer,
        "SELECT
            id,
            username,
            display_name,
            verified,
            admin
        FROM developers
        WHERE id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch developer {}: {}", id, e);
        ApiError::DbError
    })?)
}

pub async fn get_one_by_username(
    username: &str,
    conn: &mut PgConnection,
) -> Result<Option<Developer>, ApiError> {
    Ok(sqlx::query_as!(
        Developer,
        "SELECT
            id,
            username,
            display_name,
            verified,
            admin
        FROM developers
        WHERE username = $1",
        username
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch developer {}: {}", username, e);
        ApiError::DbError
    })?)
}

pub async fn get_all_for_mod(
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<Vec<ModDeveloper>, ApiError> {
    Ok(sqlx::query_as!(
        ModDeveloper,
        "SELECT
            dev.id,
            dev.username,
            dev.display_name,
            md.is_owner
        FROM developers dev
        INNER JOIN mods_developers md ON dev.id = md.developer_id
        WHERE md.mod_id = $1",
        mod_id
    )
    .fetch_all(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch developers for mod {}: {}", mod_id, e);
        ApiError::DbError
    })?)
}

pub async fn get_all_for_mods(
    mod_ids: &[String],
    conn: &mut PgConnection,
) -> Result<HashMap<String, Vec<ModDeveloper>>, ApiError> {
    if mod_ids.is_empty() {
        return Ok(HashMap::new());
    }
    #[derive(sqlx::FromRow)]
    struct QueryResult {
        pub mod_id: String,
        pub id: i32,
        pub username: String,
        pub display_name: String,
        pub is_owner: bool,
    }

    let result = sqlx::query_as!(
        QueryResult,
        "SELECT
            dev.id,
            dev.username,
            dev.display_name,
            md.is_owner,
            md.mod_id
        FROM developers dev
        INNER JOIN mods_developers md ON dev.id = md.developer_id
        WHERE md.mod_id = ANY($1)",
        mod_ids
    )
    .fetch_all(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch developers for mods: {}", e);
        ApiError::DbError
    })?;

    let mut ret = HashMap::new();

    for result_item in result {
        ret.entry(result_item.mod_id)
            .or_default()
            .push(ModDeveloper {
                id: result_item.id,
                username: result_item.username,
                display_name: result_item.display_name,
                is_owner: result_item.is_owner,
            });
    }

    Ok(ret)
}

pub async fn has_access_to_mod(
    dev_id: i32,
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<bool, ApiError> {
    Ok(sqlx::query!(
        "SELECT developer_id FROM mods_developers
        WHERE developer_id = $1
        AND mod_id = $2",
        dev_id,
        mod_id
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        log::error!(
            "Failed to find mod {} access for developer {}: {}",
            mod_id,
            dev_id,
            e
        );
        ApiError::DbError
    })?
    .is_some())
}

pub async fn owns_mod(
    dev_id: i32,
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<bool, ApiError> {
    Ok(sqlx::query!(
        "SELECT developer_id FROM mods_developers
        WHERE developer_id = $1
        AND mod_id = $2
        AND is_owner = true",
        dev_id,
        mod_id
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        log::error!(
            "Failed to check mod {} owner for developer {}: {}",
            mod_id,
            dev_id,
            e
        );
        ApiError::DbError
    })?
    .is_some())
}

pub async fn get_owner_for_mod(
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<Developer, ApiError> {
    Ok(sqlx::query_as!(
        Developer,
        "SELECT
            dev.id,
            dev.username,
            dev.display_name,
            dev.verified,
            dev.admin
        FROM developers dev
        INNER JOIN mods_developers md ON md.developer_id = dev.id
        WHERE md.mod_id = $1
        AND md.is_owner = true",
        mod_id
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => {
            log::error!("Mod {} doesn't have an owner!", mod_id);
            ApiError::InternalError
        }
        _ => {
            log::error!("Failed to fetch owner for mod {}", mod_id);
            ApiError::InternalError
        }
    })?)
}

pub async fn update_status(
    dev_id: i32,
    verified: bool,
    admin: bool,
    conn: &mut PgConnection,
) -> Result<Developer, ApiError> {
    Ok(sqlx::query_as!(
        Developer,
        "UPDATE developers
        SET admin = $1,
        verified = $2
        WHERE id = $3
        RETURNING
            id,
            username,
            display_name,
            verified,
            admin",
        admin,
        verified,
        dev_id
    )
    .fetch_one(&mut *conn)
    .map_err(|e| {
        log::error!("Failed to update developer {}: {}", dev_id, e);
        ApiError::DbError
    })?)
}

pub async fn update_profile(
    dev_id: i32,
    display_name: &str,
    conn: &mut PgConnection,
) -> Result<Developer, ApiError> {
    Ok(sqlx::query_as!(
        Developer,
        "UPDATE developers
        SET display_name = $1
        WHERE id = $2
        RETURNING
            id,
            username,
            display_name,
            verified,
            admin",
        display_name,
        dev_id
    )
    .fetch_one(&mut *conn)
    .map_err(|e| {
        log::error!("Failed to update profile for {}: {}", dev_id, e);
        ApiError::DbError
    })?)
}
