use std::collections::HashMap;

use crate::database::{repository::mod_tags, DatabaseError};
use crate::endpoints::ApiError;
use sqlx::PgConnection;
use utoipa::ToSchema;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct Tag {
    pub id: i32,
    pub name: String,
    pub display_name: String,
    pub is_readonly: bool,
}

impl Tag {
    pub async fn get_tags_for_mod(
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<Vec<String>, DatabaseError> {
        sqlx::query!(
            "SELECT mod_tags.name FROM mod_tags
            INNER JOIN mods_mod_tags ON mod_tags.id = mods_mod_tags.tag_id
            WHERE mods_mod_tags.mod_id = $1",
            mod_id
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| log::error!("{}", e))
        .map(|tags| tags.into_iter().map(|t| t.name).collect::<Vec<_>>())
        .map_err(|e| e.into())
    }

    pub async fn get_tags_for_mods(
        ids: &Vec<String>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<String, Vec<String>>, DatabaseError> {
        let tags = sqlx::query!(
            "SELECT mod_tags.name, mods_mod_tags.mod_id FROM mod_tags
            INNER JOIN mods_mod_tags ON mod_tags.id = mods_mod_tags.tag_id
            WHERE mods_mod_tags.mod_id = ANY($1)",
            ids
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| log::error!("{}", e))?;

        let mut ret: HashMap<String, Vec<String>> = HashMap::new();
        for tag in tags {
            if let Some(t) = ret.get_mut(&tag.mod_id) {
                t.push(tag.name.clone());
            } else {
                ret.insert(tag.mod_id, vec![tag.name.clone()]);
            }
        }

        Ok(ret)
    }

    pub async fn parse_tags(tags: &str, pool: &mut PgConnection) -> Result<Vec<i32>, ApiError> {
        let tags = tags
            .split(',')
            .map(|t| t.trim().to_lowercase())
            .collect::<Vec<String>>();

        let fetched = sqlx::query!(
            "SELECT DISTINCT id, name FROM mod_tags WHERE name = ANY($1)",
            &tags
        )
        .fetch_all(&mut *pool)
        .await
        .inspect_err(|e| log::error!("Failed to fetch tags: {}", e))?;

        let fetched_ids = fetched.iter().map(|t| t.id).collect::<Vec<i32>>();
        let fetched_names = fetched
            .iter()
            .map(|t| t.name.clone())
            .collect::<Vec<String>>();

        if fetched.len() != tags.len() {
            return Err(ApiError::BadRequest(format!(
                "The following tags are not allowed: '{}'",
                tags.iter()
                    .filter(|t| !fetched_names.contains(t))
                    .cloned()
                    .collect::<Vec<String>>()
                    .join(", ")
            )));
        }

        Ok(fetched_ids)
    }
}

pub async fn parse_tag_list(
    tags: &[String],
    mod_id: &str,
    conn: &mut PgConnection,
) -> Result<Vec<Tag>, ApiError> {
    if tags.is_empty() {
        return Ok(vec![]);
    }

    let db_tags = mod_tags::get_allowed_for_mod(mod_id, &mut *conn).await?;

    let mut ret = Vec::with_capacity(tags.len());
    for tag in tags {
        if let Some(t) = db_tags.iter().find(|t| t.name == *tag) {
            ret.push(t.clone());
        } else {
            let taglist = db_tags
                .into_iter()
                .map(|t| t.name)
                .collect::<Vec<String>>()
                .join(", ");

            return Err(ApiError::BadRequest(format!(
                "Tag '{}' isn't allowed. Only the following are allowed: '{}'",
                tag, taglist
            )));
        }
    }

    Ok(ret)
}
