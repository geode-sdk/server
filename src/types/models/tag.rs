use std::collections::HashMap;

use sqlx::PgConnection;

use crate::types::api::ApiError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetchedTag {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    ) -> Result<Vec<String>, ApiError> {
        let tags = match sqlx::query!(
            "SELECT mod_tags.name FROM mod_tags
            INNER JOIN mods_mod_tags ON mod_tags.id = mods_mod_tags.tag_id
            WHERE mods_mod_tags.mod_id = $1",
            mod_id
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(tags) => tags,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        Ok(tags.iter().map(|t| t.name.clone()).collect())
    }

    pub async fn get_tags_for_mods(
        ids: &Vec<String>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<String, Vec<String>>, ApiError> {
        let tags = match sqlx::query!(
            "SELECT mod_tags.name, mods_mod_tags.mod_id FROM mod_tags
            INNER JOIN mods_mod_tags ON mod_tags.id = mods_mod_tags.tag_id
            WHERE mods_mod_tags.mod_id = ANY($1)",
            ids
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(tags) => tags,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

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

        let fetched = match sqlx::query!(
            "SELECT DISTINCT id, name FROM mod_tags WHERE name = ANY($1)",
            &tags
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(fetched) => fetched,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

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
