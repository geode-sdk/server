use std::collections::{hash_map::Entry, HashMap};

use sqlx::{PgConnection, Postgres, QueryBuilder};

use crate::types::api::ApiError;

pub struct Developer {
    pub id: i32,
    pub username: String,
    pub display_name: String,
}

pub struct FetchedDeveloper {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub admin: bool,
}

pub struct ModDeveloper {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub admin: bool,
    pub is_owner: bool
}

impl Developer {
    pub async fn create(
        github_id: i64,
        username: String,
        pool: &mut PgConnection,
    ) -> Result<i32, ApiError> {
        // what the fuck github
        let username = username.trim_matches('\"');
        let result = sqlx::query!(
            "INSERT INTO developers 
            (username, display_name, github_user_id) VALUES
            ($1, $2, $3) RETURNING id",
            username.to_lowercase(),
            username.to_lowercase(),
            github_id
        )
        .fetch_one(&mut *pool)
        .await;
        let id = match result {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(row) => row.id,
        };
        Ok(id)
    }

    pub async fn get_by_github_id(
        github_id: i64,
        pool: &mut PgConnection,
    ) -> Result<Option<Developer>, ApiError> {
        let result = sqlx::query_as!(
            Developer,
            "SELECT id, username, display_name
            FROM developers WHERE github_user_id = $1",
            github_id
        )
        .fetch_optional(&mut *pool)
        .await;

        match result {
            Err(e) => {
                log::info!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(r) => Ok(r),
        }
    }

    pub async fn has_access_to_mod(
        dev_id: i32,
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<bool, ApiError> {
        let found = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mods_developers
            WHERE developer_id = $1 AND mod_id = $2",
            dev_id,
            mod_id
        )
        .fetch_one(&mut *pool)
        .await;

        match found {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(count) => Ok(count.is_some() && count.unwrap() != 0),
        }
    }

    pub async fn owns_mod(
        dev_id: i32,
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<bool, ApiError> {
        let found = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mods_developers
            WHERE developer_id = $1 AND mod_id = $2 AND is_owner = true",
            dev_id,
            mod_id
        )
        .fetch_one(&mut *pool)
        .await;

        match found {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(count) => Ok(count.is_some() && count.unwrap() != 0),
        }
    }

    pub async fn find_by_username(
        username: &str,
        pool: &mut PgConnection,
    ) -> Result<Option<FetchedDeveloper>, ApiError> {
        match sqlx::query_as!(
            FetchedDeveloper,
            "SELECT id, username, display_name, verified, admin
            FROM developers WHERE LOWER(username) = $1",
            username.to_lowercase()
        )
        .fetch_optional(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(found) => Ok(found),
        }
    }

    pub async fn fetch_for_mod(
        mod_id: &str,
        pool: &mut PgConnection,
    ) -> Result<Vec<ModDeveloper>, ApiError> {
        match sqlx::query_as!(
            ModDeveloper, 
            "SELECT dev.id, dev.username, dev.display_name, dev.verified, dev.admin, md.is_owner FROM developers dev
            INNER JOIN mods_developers md ON md.developer_id = dev.id WHERE md.mod_id = $1", 
            mod_id
        ).fetch_all(&mut *pool)
        .await 
        {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            },
            Ok(r) => Ok(r)
        }
    }

    pub async fn fetch_for_mods(
        mod_ids: Vec<String>,
        pool: &mut PgConnection
    ) -> Result<HashMap<String, Vec<ModDeveloper>>, ApiError> {
        if mod_ids.is_empty() {
            return Ok(HashMap::new());
        }
        #[derive(sqlx::FromRow)]
        struct QueryResult {
            pub mod_id: String,
            pub id: i32,
            pub username: String,
            pub display_name: String,
            pub verified: bool,
            pub admin: bool,
            pub is_owner: bool
        }

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT dev.id, dev.username, dev.display_name, dev.verified, dev.admin, md.is_owner, md.mod_id FROM developers dev 
            INNER JOIN mods_developers md ON md.developer_id = dev.id WHERE md.mod_id IN ("
        );

        let mut split = query_builder.separated(", ");
        for id in mod_ids {
            split.push_bind(id);
        }
        split.push_unseparated(")");

        let result = match query_builder.build_query_as::<QueryResult>()
            .fetch_all(&mut *pool)
            .await {
                Err(e) => {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                },
                Ok(r) => r
            };
        
        let mut ret = HashMap::new();

        for result_item in result {
            let mod_dev = ModDeveloper {
                id: result_item.id,
                username: result_item.username,
                display_name: result_item.display_name,
                verified: result_item.verified,
                admin: result_item.admin,
                is_owner: result_item.is_owner
            };
            match ret.entry(result_item.mod_id) {
                Entry::Vacant(e) => {
                    let vector: Vec<ModDeveloper> = vec![mod_dev];
                    e.insert(vector);
                }
                Entry::Occupied(mut e) => e.get_mut().push(mod_dev),
            }
        }

        Ok(ret)
    }
}
