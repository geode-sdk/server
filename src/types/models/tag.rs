use sqlx::{PgConnection, Postgres, QueryBuilder};

use crate::types::api::ApiError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetchedTag {
    pub id: i32,
    pub name: String,
}

pub struct Tag;

impl Tag {
    pub async fn get_tag_ids(
        tags: Vec<String>,
        pool: &mut PgConnection,
    ) -> Result<Vec<FetchedTag>, ApiError> {
        let db_tags = match sqlx::query_as!(FetchedTag, "SELECT id, name FROM mod_tags")
            .fetch_all(&mut *pool)
            .await
        {
            Ok(tags) => tags,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        let mut ret = Vec::new();
        for tag in tags {
            if let Some(t) = db_tags.iter().find(|t| t.name == tag.to_lowercase()) {
                ret.push(t.clone())
            } else {
                return Err(ApiError::BadRequest(format!(
                    "Tag '{}' isn't allowed. Only the following are allowed: '{}'",
                    tag,
                    db_tags
                        .iter()
                        .map(|t| t.name.clone())
                        .collect::<Vec<String>>()
                        .join(", ")
                )));
            }
        }

        Ok(ret)
    }

    pub async fn add_tags_to_mod_version(
        mod_version_id: i32,
        tags: Vec<i32>,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("INSERT INTO mods_mod_tags (mod_id, tag_id) VALUES (");

        for (index, tag) in tags.iter().enumerate() {
            let mut separated = query_builder.separated(", ");
            separated.push_bind(mod_version_id);
            separated.push_bind(tag);
            separated.push(")");

            if index != tags.len() - 1 {
                separated.push(", (");
            }
        }

        if let Err(e) = query_builder.build().execute(&mut *pool).await {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(())
    }
}
