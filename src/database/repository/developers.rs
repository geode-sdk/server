use crate::types::api::ApiError;
use crate::types::models::developer::FetchedDeveloper;
use sqlx::PgConnection;

pub async fn fetch_or_insert_github(
    github_id: i64,
    username: &str,
    conn: &mut PgConnection,
) -> Result<FetchedDeveloper, ApiError> {
    match sqlx::query_as!(
        FetchedDeveloper,
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
) -> Result<FetchedDeveloper, ApiError> {
    Ok(sqlx::query_as!(
        FetchedDeveloper,
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

pub async fn get_owner_for_mod(
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<FetchedDeveloper, ApiError> {
    Ok(sqlx::query_as!(
        FetchedDeveloper,
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
