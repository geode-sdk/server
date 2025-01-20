use crate::database::repository::developers::get_all_for_mod;
use crate::types::api::ApiError;
use crate::types::models::tag::Tag;
use sqlx::{PgConnection, Postgres, QueryBuilder};

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

pub async fn get_all_from_names(
    names: &[String],
    conn: &mut PgConnection,
) -> Result<Vec<Tag>, ApiError> {
    let tags = sqlx::query!(
        "SELECT
            id,
            name,
            display_name,
            is_readonly
        FROM mod_tags
        WHERE name = ANY($1)",
        names
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("mod_tags::get_all_from_names failed: {}", e);
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

pub async fn get_for_mod(mod_id: &str, conn: &mut PgConnection) -> Result<Vec<Tag>, ApiError> {
    Ok(sqlx::query!(
        "SELECT
            mt.id,
            mt.name,
            mt.display_name,
            mt.is_readonly
        FROM mod_tags mt
        INNER JOIN mods_mod_tags mmt ON mmt.tag_id = mt.id
        WHERE mmt.mod_id = $1",
        mod_id
    )
    .fetch_all(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch mod_tags for mod: {}", e);
        ApiError::DbError
    })?
    .into_iter()
    .map(|i| Tag {
        id: i.id,
        display_name: i.display_name.unwrap_or(i.name.clone()),
        name: i.name,
        is_readonly: i.is_readonly,
    })
    .collect::<Vec<Tag>>())
}

pub async fn update_for_mod(
    mod_id: &str,
    new_tags: &[i32],
    conn: &mut PgConnection,
) -> Result<(), ApiError> {
    let existing = get_for_mod(mod_id, conn).await?;

    let insertable = new_tags
        .iter()
        .filter(|t| !existing.iter().any(|e| e.id == **t))
        .collect::<Vec<i32>>();

    let deletable = existing
        .iter()
        .filter(|e| !new_tags.iter().any(|t| e.id == *t))
        .map(|x| x.id)
        .collect::<Vec<i32>>();

    sqlx::query!(
        "DELETE FROM mods_mod_tags
        WHERE mod_id = $1
        AND tag_id = ANY($2)",
        mod_id,
        &deletable
    )
    .execute(conn)
    .await
    .map_err(|e| {
        log::error!("Failed to delete mod tags: {}", e);
        ApiError::DbError
    })?;

    let mut builder: QueryBuilder<Postgres> =
        QueryBuilder::new("INSERT INTO mods_mod_tags (mod_id, tag_id) VALUES ");

    for (i, tag) in insertable.into_iter().enumerate() {
        builder.push("(");
        let mut separated = builder.separated(", ");
        separated.push_bind(mod_id);
        separated.push_bind(tag);
        builder.push(")");

        if i != insertable.len() - 1 {
            builder.push(", ");
        }
    }

    builder.build().execute(conn).await.map_err(|e| {
        log::error!("Failed to insert tags for mod: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}
