use sqlx::PgConnection;

use crate::types::api::ApiError;

pub struct Developer {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub github_user_id: i64
}

impl Developer {
    pub async fn create(github_id: i64, username: String, pool: &mut PgConnection) -> Result<i32, ApiError> {
        let result = sqlx::query!(
            "INSERT INTO developers 
            (username, display_name, github_user_id) VALUES
            ($1, $2, $3) RETURNING id",
            username,
            username,
            github_id
        ).fetch_one(&mut *pool).await;
        let id = match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            },
            Ok(row) => row.id
        };
        Ok(id)
    }

    pub async fn get_by_github_id(github_id: i64, pool: &mut PgConnection) -> Result<Option<Developer>, ApiError> {
        let result = sqlx::query_as!(
            Developer,
            "SELECT id, username, display_name, verified, github_user_id
            FROM developers WHERE github_user_id = $1",
            github_id
        ).fetch_optional(&mut *pool).await;

        match result {
            Err(e) => {
                log::info!("{}", e);
                return Err(ApiError::DbError);
            },
            Ok(r) => Ok(r)
        }
    }
}