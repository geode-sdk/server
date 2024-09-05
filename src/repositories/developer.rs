use sqlx::PgConnection;

use crate::types::{api::ApiError, models::developer::Developer};

pub struct CreateDto {
    pub github_id: i64,
    pub username: String,
}

pub async fn create(data: CreateDto, connection: &mut PgConnection) -> Result<Developer, ApiError> {
    let username = data.username.trim_matches('\"');

    sqlx::query!(
        "
            INSERT INTO developers
            (username, display_name, github_user_id) VALUES
            ($1, $2, $3) 
            RETURNING
                id,
                username,
                display_name,
                verified,
                admin
        ",
        username,
        data.username,
        data.github_id
    )
    .fetch_one(connection)
    .await
    .map(|x| Developer {
        id: x.id,
        username: x.username,
        display_name: x.display_name,
        verified: x.verified,
        admin: x.admin,
        is_owner: None,
    })
    .map_err(|x| {
        log::error!("{}", x);
        ApiError::DbError
    })
}
