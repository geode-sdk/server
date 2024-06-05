use std::collections::{hash_map::Entry, HashMap};

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, QueryBuilder};

use crate::types::api::{ApiError, PaginatedData};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Developer {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub is_owner: bool,
}

#[derive(Serialize, Clone)]
pub struct DeveloperProfile {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub admin: bool,
}

#[derive(sqlx::FromRow, Clone)]
pub struct FetchedDeveloper {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub verified: bool,
    pub admin: bool,
}

impl Developer {
    pub async fn get_index(
        query: &Option<String>,
        page: i64,
        per_page: i64,
        pool: &mut PgConnection,
    ) -> Result<PaginatedData<DeveloperProfile>, ApiError> {
        let limit = per_page;
        let offset = (page - 1) * per_page;

        let name_query: Option<String> = query.as_ref().map(|q| format!("%{}%", q));

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT
                id,
                username,
                display_name,
                verified,
                admin     
            FROM developers
            "#,
        );

        let mut counter: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT COUNT(id)
            FROM developers
        "#,
        );

        if name_query.is_some() {
            let sql = "WHERE username LIKE ";
            builder.push(sql);
            counter.push(sql);
            builder.push_bind(name_query.clone().unwrap());
            counter.push(name_query.clone().unwrap());
            let sql = " OR WHERE display_name LIKE ";
            builder.push(sql);
            counter.push(sql);
            builder.push(name_query.clone().unwrap());
            counter.push(name_query.clone().unwrap());
        }

        builder.push(" GROUP BY id");
        let sql = " LIMIT ";
        builder.push(sql);
        builder.push_bind(limit);
        let sql = " OFFSET ";
        builder.push(sql);
        builder.push_bind(offset);

        let result = match builder
            .build_query_as::<FetchedDeveloper>()
            .fetch_all(&mut *pool)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        let result: Vec<DeveloperProfile> = result
            .into_iter()
            .map(|x| DeveloperProfile {
                id: x.id,
                username: x.username,
                display_name: x.display_name,
                verified: x.verified,
                admin: x.admin,
            })
            .collect();

        let count = match counter
            .build_query_scalar()
            .fetch_optional(&mut *pool)
            .await
        {
            Ok(Some(c)) => c,
            Ok(None) => 0,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        Ok(PaginatedData {
            data: result,
            count,
        })
    }

    pub async fn get_one(
        id: i32,
        pool: &mut PgConnection,
    ) -> Result<Option<FetchedDeveloper>, ApiError> {
        let result = match sqlx::query_as!(
            FetchedDeveloper,
            "SELECT
                id,
                username,
                display_name,
                verified,
                admin
            FROM developers
            WHERE id = $1
            ",
            id
        )
        .fetch_optional(&mut *pool)
        .await
        {
            Ok(d) => d,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };

        Ok(result)
    }

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
    ) -> Result<Option<FetchedDeveloper>, ApiError> {
        let result = sqlx::query_as!(
            FetchedDeveloper,
            "SELECT id, username, display_name, verified, admin
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
    ) -> Result<Vec<Developer>, ApiError> {
        match sqlx::query_as!(
            Developer,
            "SELECT dev.id, dev.username, dev.display_name, md.is_owner FROM developers dev
            INNER JOIN mods_developers md ON md.developer_id = dev.id WHERE md.mod_id = $1",
            mod_id
        )
        .fetch_all(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
            Ok(r) => Ok(r),
        }
    }

    pub async fn fetch_for_mods(
        mod_ids: &Vec<String>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<String, Vec<Developer>>, ApiError> {
        if mod_ids.is_empty() {
            return Ok(HashMap::new());
        }
        #[derive(sqlx::FromRow)]
        struct QueryResult {
            pub mod_id: String,
            pub id: i32,
            pub username: String,
            pub display_name: String,
            pub is_owner: bool,
        }

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT dev.id, dev.username, dev.display_name, dev.verified, md.is_owner, md.mod_id FROM developers dev 
            INNER JOIN mods_developers md ON md.developer_id = dev.id WHERE md.mod_id IN ("
        );

        let mut split = query_builder.separated(", ");
        for id in mod_ids {
            split.push_bind(id);
        }
        split.push_unseparated(")");

        let result = match query_builder
            .build_query_as::<QueryResult>()
            .fetch_all(&mut *pool)
            .await
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        let mut ret = HashMap::new();

        for result_item in result {
            let mod_dev = Developer {
                id: result_item.id,
                username: result_item.username,
                display_name: result_item.display_name,
                is_owner: result_item.is_owner,
            };
            match ret.entry(result_item.mod_id) {
                Entry::Vacant(e) => {
                    let vector: Vec<Developer> = vec![mod_dev];
                    e.insert(vector);
                }
                Entry::Occupied(mut e) => e.get_mut().push(mod_dev),
            }
        }

        Ok(ret)
    }

    pub async fn update_profile(
        id: i32,
        display_name: &str,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let str = String::from(display_name);
        if !str.chars().all(char::is_alphanumeric) {
            return Err(ApiError::BadRequest(
                "Display name must contain only alphanumeric characters".to_string(),
            ));
        }

        let result = match sqlx::query!(
            "UPDATE developers SET display_name = $1 WHERE id = $2",
            display_name,
            id
        )
        .execute(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        if result.rows_affected() == 0 {
            return Err(ApiError::InternalError);
        }

        Ok(())
    }

    pub async fn update(
        id: i32,
        admin: Option<bool>,
        verified: Option<bool>,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new("update developers set ");
        if let Some(a) = admin {
            query.push("admin = ");
            query.push_bind(a);
            if verified.is_some() {
                query.push(", ");
            }
        }
        if let Some(v) = verified {
            query.push("verified = ");
            query.push_bind(v);
        }
        query.push(" where id = ");
        query.push_bind(id);

        let result = match query.build().execute(&mut *pool).await {
            Ok(r) => r,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };
        if result.rows_affected() == 0 {
            return Err(ApiError::DbError);
        }
        Ok(())
    }
}
