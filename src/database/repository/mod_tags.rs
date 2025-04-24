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

pub async fn get_for_mod(id: &str, conn: &mut PgConnection) -> Result<Vec<Tag>, ApiError> {
    sqlx::query!(
        "SELECT
            id,
            name,
            display_name,
            is_readonly
        FROM mod_tags mt
        INNER JOIN mods_mod_tags mmt ON mmt.tag_id = mt.id
        WHERE mmt.mod_id = $1",
        id
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| {
        log::error!("mod_tags::get_tags failed: {}", e);
        ApiError::DbError
    })
    .map(|vec| {
        vec.into_iter()
            .map(|i| Tag {
                id: i.id,
                display_name: i.display_name.unwrap_or(i.name.clone()),
                name: i.name,
                is_readonly: i.is_readonly,
            })
            .collect()
    })
}

pub async fn parse_tag_list(
    tags: &[String],
    conn: &mut PgConnection,
) -> Result<Vec<Tag>, ApiError> {
    if tags.is_empty() {
        return Ok(vec![]);
    }

    let db_tags = get_all(conn).await?;

    let mut ret = Vec::new();
    for tag in tags {
        if let Some(t) = db_tags.iter().find(|t| t.name == *tag) {
            ret.push(t.clone());
        } else {
            return Err(ApiError::BadRequest(format!(
                "Tag '{}' isn't allowed. Only the following are allowed: '{}'",
                tag,
                db_tags
                    .into_iter()
                    .map(|t| t.name)
                    .collect::<Vec<String>>()
                    .join(", ")
            )));
        }
    }

    Ok(ret)
}

pub async fn update_for_mod(
    id: &str,
    tags: &[Tag],
    conn: &mut PgConnection,
) -> Result<(), ApiError> {
    let existing = get_for_mod(id, &mut *conn).await?;

    let insertable = tags
        .iter()
        .filter(|t| !existing.iter().any(|e| e.id == t.id))
        .map(|x| x.id)
        .collect::<Vec<_>>();

    let deletable = existing
        .iter()
        .filter(|e| !tags.iter().any(|t| e.id == t.id))
        .map(|x| x.id)
        .collect::<Vec<_>>();

    if !deletable.is_empty() {
        sqlx::query!(
            "DELETE FROM mods_mod_tags
            WHERE mod_id = $1
            AND tag_id = ANY($2)",
            id,
            &deletable
        )
        .execute(&mut *conn)
        .await
        .inspect_err(|e| log::error!("Failed to remove tags: {}", e))
        .or(Err(ApiError::DbError))?;
    }

    if !insertable.is_empty() {
        let mod_id = vec![id.into(); insertable.len()];

        sqlx::query!(
            "INSERT INTO mods_mod_tags
                (mod_id, tag_id)
            SELECT * FROM UNNEST(
                $1::text[],
                $2::int4[]
            )",
            &mod_id,
            &insertable
        )
        .execute(&mut *conn)
        .await
        .inspect_err(|e| log::error!("Failed to insert tags: {}", e))
        .or(Err(ApiError::DbError))?;
    }

    Ok(())
}
