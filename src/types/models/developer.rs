use sqlx::PgConnection;

use crate::types::api::ApiError;

pub struct Developer {
    pub id: i32,
    pub username: String,
    pub display_name: String,
}

pub struct FetchedDeveloper {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub admin: bool,
}

impl Developer {
    pub async fn create(
        github_id: i64,
        username: String,
        pool: &mut PgConnection,
    ) -> Result<i32, ApiError> {
        // what the fuck github
        let username = username.trim_matches('\"');
        let result = sqlx::query!(
            "INSERT INTO developers 
            (username, display_name, github_user_id) VALUES
            ($1, $2, $3) RETURNING id",
            username.to_lowercase(),
            username.to_lowercase(),
            github_id
        )
        .fetch_one(&mut *pool)
        .await;
        let id = match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(row) => row.id,
        };
        Ok(id)
    }

    pub async fn get_by_github_id(
        github_id: i64,
        pool: &mut PgConnection,
    ) -> Result<Option<Developer>, ApiError> {
        let result = sqlx::query_as!(
            Developer,
            "SELECT id, username, display_name
            FROM developers WHERE github_user_id = $1",
            github_id
        )
        .fetch_optional(&mut *pool)
        .await;

        match result {
            Err(e) => {
                log::info!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(r) => Ok(r),
        }
    }

    pub async fn has_access_to_mod(
        dev_id: i32,
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<bool, ApiError> {
        let found = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mods_developers
            WHERE developer_id = $1 AND mod_id = $2",
            dev_id,
            mod_id
        )
        .fetch_one(&mut *pool)
        .await;

        match found {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(count) => Ok(count.is_some() && count.unwrap() != 0),
        }
    }

    pub async fn owns_mod(
        dev_id: i32,
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<bool, ApiError> {
        let found = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mods_developers
            WHERE developer_id = $1 AND mod_id = $2 AND is_lead = true",
            dev_id,
            mod_id
        )
        .fetch_one(&mut *pool)
        .await;

        match found {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(count) => Ok(count.is_some() && count.unwrap() != 0),
        }
    }

    pub async fn find_by_username(
        username: &str,
        pool: &mut PgConnection,
    ) -> Result<Option<FetchedDeveloper>, ApiError> {
        match sqlx::query_as!(
            FetchedDeveloper,
            "SELECT id, username, display_name, verified, admin
            FROM developers WHERE LOWER(username) = $1",
            username.to_lowercase()
        )
        .fetch_optional(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(found) => Ok(found),
        }
    }
}
