use crate::types::models::dependency::ModVersionCompare;
use serde::{Serialize, Deserialize};
use sqlx::{QueryBuilder, Postgres, PgConnection};
use crate::types::api::ApiError;


#[derive(sqlx::FromRow, Clone, Debug)]
pub struct FetchedIncompatibility {
    pub mod_id: String,
    pub version: String,
    pub incompatibility_id: i32,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance 
}

pub struct IncompatibilityCreate {
    pub incompatibility_id: i32,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance
}

#[derive(sqlx::FromRow)]
pub struct Incompatibility {
    pub mod_id: i32,
    pub incompatibility_id: i32,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance 
}

#[derive(sqlx::Type, Debug, Serialize, Clone, Copy, Deserialize)]
#[sqlx(type_name = "dependency_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum IncompatibilityImportance {
    Breaking,
    Conflicting
}

#[derive(Serialize, Debug, Clone)]
pub struct ResponseIncompatibility {
    pub mod_id: String,
    pub version: String,
    pub importance: IncompatibilityImportance 
}

impl Incompatibility {
    pub async fn create_for_mod_version(id: i32, incompats: Vec<IncompatibilityCreate>, pool: &mut PgConnection) -> Result<(), ApiError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO incompatibilities (mod_id, incompatible_id, compare, importance) VALUES ");
        let mut index = 0;
        for i in &incompats {
            let mut separated = builder.separated(", ");
            separated.push_unseparated("(");
            separated.push_bind(id);
            separated.push_bind(i.incompatibility_id);
            separated.push_bind(i.compare);
            separated.push_bind(i.importance);
            log::info!("{}", index);
            separated.push_unseparated(")");
            if index != incompats.len() - 1 {
                separated.push_unseparated(", ");
            }
            index += 1;
        }

        let result = builder.build().execute(&mut *pool).await;
        if result.is_err() {
            log::error!("{:?}", result.err().unwrap());
            return Err(ApiError::DbError);
        }

        Ok(())
    }

    pub async fn get_for_mod_version(id: i32, pool: &mut PgConnection) -> Result<Vec<FetchedIncompatibility>, ApiError> {
        let result = sqlx::query_as!(FetchedIncompatibility,
            r#"SELECT icp.compare as "compare: ModVersionCompare",
            icp.importance as "importance: IncompatibilityImportance",
            icp.incompatibility_id, mv.mod_id, mv.version FROM incompatibilities icp
            INNER JOIN mod_versions mv ON icp.mod_id = mv.id
            WHERE mv.id = $1"#, id
        ).fetch_all(&mut *pool)
        .await;
        if result.is_err() {
            log::info!("{}", result.err().unwrap());
            return Err(ApiError::DbError);
        }
        Ok(result.unwrap())
    }
}