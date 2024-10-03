use std::collections::HashMap;

use sqlx::{PgConnection, Postgres, QueryBuilder};

use crate::types::api::ApiError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetchedTag {
    pub id: i32,
    pub name: String,
}

pub struct Tag;

impl Tag {
    pub async fn get_tags(pool: &mut PgConnection) -> Result<Vec<String>, ApiError> {
        let tags = match sqlx::query!("SELECT name FROM mod_tags")
            .fetch_all(&mut *pool)
            .await
        {
            Ok(tags) => tags,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        Ok(tags.into_iter().map(|x| x.name).collect::<Vec<String>>())
    }

    pub async fn get_tag_ids(
        tags: Vec<String>,
        pool: &mut PgConnection,
    ) -> Result<Vec<FetchedTag>, ApiError> {
        let db_tags = match sqlx::query_as!(
            FetchedTag,
            "SELECT id, name FROM mod_tags WHERE readonly = false"
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

    pub async fn update_mod_tags(
        mod_id: &str,
        tags: Vec<i32>,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let existing = match sqlx::query!(
            "SELECT mod_id, tag_id FROM mods_mod_tags WHERE mod_id = $1",
            mod_id,
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(existing) => existing,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        let insertable = tags
            .iter()
            .filter(|t| !existing.iter().any(|e| e.tag_id == **t))
            .collect::<Vec<_>>();

        let deletable = existing
            .iter()
            .filter(|e| !tags.iter().any(|t| e.tag_id == *t))
            .map(|x| x.tag_id)
            .collect::<Vec<_>>();

        Tag::delete_tags_for_mod(mod_id, deletable, pool).await?;

        if insertable.is_empty() {
            return Ok(());
        }

        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("INSERT INTO mods_mod_tags (mod_id, tag_id) VALUES (");

        for (index, tag) in insertable.iter().enumerate() {
            if existing.iter().any(|e| e.tag_id == **tag) {
                continue;
            }
            let mut separated = query_builder.separated(", ");
            separated.push_bind(mod_id);
            separated.push_bind(tag);
            query_builder.push(")");

            if index != insertable.len() - 1 {
                query_builder.push(", (");
            }
        }

        if let Err(e) = query_builder.build().execute(&mut *pool).await {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(())
    }

    pub async fn delete_tags_for_mod(
        mod_id: &str,
        tags: Vec<i32>,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        if tags.is_empty() {
            return Ok(());
        }
        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("DELETE FROM mods_mod_tags WHERE (mod_id, tag_id) IN ((");

        for (index, tag) in tags.iter().enumerate() {
            let mut separated = query_builder.separated(", ");
            separated.push_bind(mod_id);
            separated.push_bind(tag);
            query_builder.push(")");

            if index != tags.len() - 1 {
                query_builder.push(", (");
            }
        }
        query_builder.push(")");

        if let Err(e) = query_builder.build().execute(&mut *pool).await {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
        Ok(())
    }

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
