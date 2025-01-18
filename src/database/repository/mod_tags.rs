use crate::types::api::ApiError;
use crate::types::models::tag::Tag;
use sqlx::PgConnection;

pub async fn get_all(conn: &mut PgConnection) -> Result<Vec<Tag>, ApiError> {
    let tags = sqlx::query!(
        "SELECT
            id,
            name,
            display_name,
            is_readonly
        FROM mod_tags"
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("mod_tags::get_tags failed: {}", e);
        ApiError::DbError
    })?
    .into_iter()
    .map(|i| Tag {
        id: i.id,
        display_name: i.display_name.unwrap_or(i.name.clone()),
        name: i.name,
        is_readonly: i.is_readonly,
    })
    .collect::<Vec<Tag>>();

    Ok(tags)
}
