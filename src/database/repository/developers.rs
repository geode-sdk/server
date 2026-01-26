use crate::database::DatabaseError;
use crate::types::api::PaginatedData;
use crate::types::models::developer::{Developer, ModDeveloper};
use sqlx::PgConnection;
use std::collections::HashMap;
use uuid::Uuid;

pub async fn index(
    query: Option<&str>,
    page: i64,
    per_page: i64,
    conn: &mut PgConnection,
) -> Result<PaginatedData<Developer>, DatabaseError> {
    let limit = per_page;
    let offset = (page - 1) * per_page;

    let result = sqlx::query_as!(
        Developer,
        "SELECT
            id,
            username,
            display_name,
            verified,
            admin,
            github_user_id as github_id
        FROM developers
        WHERE (
            ($1::text IS NULL OR username = $1)
            OR ($1::text IS NULL OR display_name ILIKE '%' || $1 || '%')
        )
        GROUP BY id
        LIMIT $2
        OFFSET $3",
        query,
        limit,
        offset
    )
    .fetch_all(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to fetch developers: {}", e))?;

    let count = index_count(query, &mut *conn).await?;

    Ok(PaginatedData {
        data: result,
        count,
    })
}

pub async fn index_count(
    query: Option<&str>,
    conn: &mut PgConnection,
) -> Result<i64, DatabaseError> {
    sqlx::query!(
        "SELECT COUNT(id)
        FROM developers
        WHERE (
            ($1::text IS NULL OR username = $1)
            OR ($1::text IS NULL OR display_name ILIKE '%' || $1 || '%')
        )",
        query
    )
    .fetch_one(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to fetch developer count: {}", e))
    .map(|x| x.count.unwrap_or(0))
    .map_err(|e| e.into())
}

pub async fn fetch_or_insert_github(
    github_id: i64,
    username: &str,
    conn: &mut PgConnection,
) -> Result<Developer, DatabaseError> {
    match sqlx::query_as!(
        Developer,
        "SELECT
            id,
            username,
            display_name,
            verified,
            admin,
            github_user_id as github_id
        FROM developers
        WHERE github_user_id = $1",
        github_id
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to fetch developer for GitHub id: {e}"))?
    {
        Some(dev) => Ok(dev),
        None => Ok(insert_github(github_id, username, conn).await?),
    }
}

async fn insert_github(
    github_id: i64,
    username: &str,
    conn: &mut PgConnection,
) -> Result<Developer, DatabaseError> {
    sqlx::query_as!(
        Developer,
        "INSERT INTO developers(username, display_name, github_user_id)
        VALUES ($1, $1, $2)
        RETURNING
            id,
            username,
            display_name,
            verified,
            admin,
            github_user_id as github_id",
        username,
        github_id
    )
    .fetch_one(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to insert developer: {e}"))
    .map_err(|e| e.into())
}

pub async fn get_one(id: i32, conn: &mut PgConnection) -> Result<Option<Developer>, DatabaseError> {
    sqlx::query_as!(
        Developer,
        "SELECT
            id,
            username,
            display_name,
            verified,
            admin,
            github_user_id as github_id
        FROM developers
        WHERE id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to fetch developer {id}: {e}"))
    .map_err(|e| e.into())
}

pub async fn get_one_by_username(
    username: &str,
    conn: &mut PgConnection,
) -> Result<Option<Developer>, DatabaseError> {
    sqlx::query_as!(
        Developer,
        "SELECT
            id,
            username,
            display_name,
            verified,
            admin,
            github_user_id as github_id
        FROM developers
        WHERE username = $1
        OR ( display_name ILIKE '%' || $1 || '%' OR username ILIKE '%' || $1 || '%' )
        ORDER BY
            CASE
                WHEN username = $1 then 1
                else 0
            END DESC
        LIMIT 1",
        username
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to fetch developer {username}: {e}"))
    .map_err(|x| x.into())
}

pub async fn get_all_for_mod(
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<Vec<ModDeveloper>, DatabaseError> {
    sqlx::query_as!(
        ModDeveloper,
        "SELECT
            dev.id,
            dev.username,
            dev.display_name,
            md.is_owner
        FROM developers dev
        INNER JOIN mods_developers md ON dev.id = md.developer_id
        WHERE md.mod_id = $1
        ORDER BY md.is_owner DESC, dev.id ASC",
        mod_id
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| log::error!("Failed to fetch developers for mod {}: {}", mod_id, e))
    .map_err(|e| e.into())
}

pub async fn get_all_for_mods(
    mod_ids: &[String],
    conn: &mut PgConnection,
) -> Result<HashMap<String, Vec<ModDeveloper>>, DatabaseError> {
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
    .inspect_err(|e| log::error!("Failed to fetch developers for mods: {}", e))?;

    let mut ret: HashMap<String, Vec<ModDeveloper>> = HashMap::new();

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
) -> Result<bool, DatabaseError> {
    sqlx::query!(
        "SELECT developer_id FROM mods_developers
        WHERE developer_id = $1
        AND mod_id = $2",
        dev_id,
        mod_id
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| {
        log::error!(
            "Failed to find mod {} access for developer {}: {}",
            mod_id,
            dev_id,
            e
        );
    })
    .map(|x| x.is_some())
    .map_err(|e| e.into())
}

pub async fn owns_mod(
    dev_id: i32,
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<bool, DatabaseError> {
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
    .inspect_err(|e| {
        log::error!(
            "Failed to check mod {} owner for developer {}: {}",
            mod_id,
            dev_id,
            e
        )
    })?
    .is_some())
}

pub async fn get_owner_for_mod(
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<Option<Developer>, DatabaseError> {
    sqlx::query_as!(
        Developer,
        "SELECT
            dev.id,
            dev.username,
            dev.display_name,
            dev.verified,
            dev.admin,
            github_user_id as github_id
        FROM developers dev
        INNER JOIN mods_developers md ON md.developer_id = dev.id
        WHERE md.mod_id = $1
        AND md.is_owner = true",
        mod_id
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to fetch owner for mod {mod_id}: {e}"))
    .map_err(|e| e.into())
}

pub async fn update_status(
    dev_id: i32,
    verified: bool,
    admin: bool,
    conn: &mut PgConnection,
) -> Result<Developer, DatabaseError> {
    sqlx::query_as!(
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
            admin,
            github_user_id as github_id",
        admin,
        verified,
        dev_id
    )
    .fetch_one(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to update developer {dev_id}: {e}"))
    .map_err(|e| e.into())
}

pub async fn update_profile(
    dev_id: i32,
    display_name: &str,
    conn: &mut PgConnection,
) -> Result<Developer, DatabaseError> {
    sqlx::query_as!(
        Developer,
        "UPDATE developers
        SET display_name = $1
        WHERE id = $2
        RETURNING
            id,
            username,
            display_name,
            verified,
            admin,
            github_user_id as github_id",
        display_name,
        dev_id
    )
    .fetch_one(&mut *conn)
    .await
    .inspect_err(|e| log::error!("Failed to update profile for {dev_id}: {e}"))
    .map_err(|e| e.into())
}

pub async fn find_by_refresh_token(
    uuid: Uuid,
    conn: &mut PgConnection,
) -> Result<Option<Developer>, DatabaseError> {
    let hash = sha256::digest(uuid.to_string());
    sqlx::query_as!(
        Developer,
        "SELECT
            d.id,
            d.username,
            d.display_name,
            d.admin,
            d.verified,
            d.github_user_id as github_id
        FROM developers d
        INNER JOIN refresh_tokens rt ON d.id = rt.developer_id
        WHERE rt.token = $1
        AND rt.expires_at > NOW()",
        hash
    )
    .fetch_optional(conn)
    .await
    .inspect_err(|e| log::error!("Failed to search for developer by refresh token: {e}"))
    .map_err(|e| e.into())
}

pub async fn find_by_token(
    token: &Uuid,
    conn: &mut PgConnection,
) -> Result<Option<Developer>, DatabaseError> {
    let hash = sha256::digest(token.to_string());
    sqlx::query_as!(
        Developer,
        "SELECT
            d.id,
            d.username,
            d.display_name,
            d.verified,
            d.admin,
            d.github_user_id as github_id
        FROM developers d
        INNER JOIN auth_tokens a ON d.id = a.developer_id
        WHERE a.token = $1
        AND (
            expires_at IS NULL
            OR expires_at > NOW()
        )",
        hash
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| log::error!("{}", e))
    .map_err(|e| e.into())
}
