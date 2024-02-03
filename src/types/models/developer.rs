use sqlx::PgConnection;

use crate::types::api::ApiError;

pub struct Developer {
    id: i32,
    username: String,
    display_name: String,
    verified: bool,
    github_user_id: i64
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
}