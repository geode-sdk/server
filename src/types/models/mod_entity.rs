use serde::Serialize;
use sqlx::PgConnection;
use crate::{types::{models::mod_version::ModVersion, api::PaginatedData}, Error};

#[derive(Serialize, Debug, sqlx::FromRow)]
pub struct Mod {
    pub id: String,
    pub repository: String,
    pub latest_version: String,
    pub validated: bool,
    pub versions: Vec<ModVersion>
}

impl Mod {
    pub async fn get_index(pool: &mut PgConnection, page: i64, per_page: i64, query: String) -> Result<PaginatedData<Mod>, Error> {
    #[derive(Debug)]
    struct ModRecord {
        id: String,
        repository: String,
        latest_version: String,
        validated: bool,
    }
        let limit = per_page;
        let offset = (page - 1) * per_page;
        let mut query_string = "%".to_owned();
        query_string.push_str(query.as_str());
        query_string.push_str("%");
        let records: Vec<ModRecord> = sqlx::query_as!(ModRecord, r#"SELECT * FROM mods WHERE id LIKE $1 LIMIT $2 OFFSET $3"#, query_string, limit, offset)
            .fetch_all(&mut *pool)
            .await.or(Err(Error::DbError))?;
        let count = sqlx::query_scalar!("SELECT COUNT(*) FROM mods")
            .fetch_one(&mut *pool)
            .await.or(Err(Error::DbError))?.unwrap_or(0);

        let ids = records.iter().map(|x| x.id.as_str()).collect();
        let versions = ModVersion::get_versions_for_mods(pool, ids).await?;

        let ret = records.into_iter().map(|x| {
            let version_vec = versions.get(&x.id).map(|x| x.clone()).unwrap_or_default();
            Mod {
                id: x.id.clone(),
                repository: x.repository.clone(),
                latest_version: x.latest_version.clone(),
                validated: x.validated,
                versions: version_vec
            }
        }).collect();
        Ok(PaginatedData{ data: ret, count })
    }
}