use std::collections::HashMap;

use crate::types::api::ApiError;
use crate::types::models::dependency::ModVersionCompare;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, QueryBuilder};

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct FetchedIncompatibility {
    pub mod_id: i32,
    pub version: String,
    pub incompatibility_id: String,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance,
}

pub struct IncompatibilityCreate {
    pub incompatibility_id: String,
    pub version: String,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance,
}

#[derive(sqlx::FromRow)]
pub struct Incompatibility {
    pub mod_id: i32,
    pub incompatibility_id: String,
    pub compare: ModVersionCompare,
    pub importance: IncompatibilityImportance,
}

#[derive(sqlx::Type, Debug, Serialize, Clone, Copy, Deserialize, PartialEq)]
#[sqlx(type_name = "incompatibility_importance", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum IncompatibilityImportance {
    Breaking,
    Conflicting,
    Superseded,
}

#[derive(Serialize, Debug, Clone)]
pub struct ResponseIncompatibility {
    pub mod_id: String,
    pub version: String,
    pub importance: IncompatibilityImportance,
}

impl FetchedIncompatibility {
    pub fn to_response(&self) -> ResponseIncompatibility {
        ResponseIncompatibility {
            mod_id: self.incompatibility_id.clone(),
            version: {
                if self.version == "*" {
                    "*".to_string()
                } else {
                    format!("{}{}", self.compare, self.version)
                }
            },
            importance: self.importance,
        }
    }
}

impl Incompatibility {
    pub async fn create_for_mod_version(
        id: i32,
        incompats: Vec<IncompatibilityCreate>,
        pool: &mut PgConnection,
    ) -> Result<(), ApiError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO incompatibilities (mod_id, incompatibility_id, version, compare, importance) VALUES ",
        );
        for (index, i) in incompats.iter().enumerate() {
            let mut separated = builder.separated(", ");
            separated.push_unseparated("(");
            separated.push_bind(id);
            separated.push_bind(&i.incompatibility_id);
            separated.push_bind(&i.version);
            separated.push_bind(i.compare);
            separated.push_bind(i.importance);
            separated.push_unseparated(")");
            if index != incompats.len() - 1 {
                separated.push_unseparated(", ");
            }
        }

        let result = builder.build().execute(&mut *pool).await;
        if result.is_err() {
            log::error!("{:?}", result.err().unwrap());
            return Err(ApiError::DbError);
        }

        Ok(())
    }

    pub async fn get_for_mod_version(
        id: i32,
        pool: &mut PgConnection,
    ) -> Result<Vec<FetchedIncompatibility>, ApiError> {
        match sqlx::query_as!(
            FetchedIncompatibility,
            r#"SELECT icp.compare as "compare: _",
            icp.importance as "importance: _",
            icp.incompatibility_id, icp.mod_id, icp.version FROM incompatibilities icp
            INNER JOIN mod_versions mv ON mv.id = icp.mod_id
            WHERE mv.id = $1"#,
            id
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(d) => Ok(d),
            Err(e) => {
                log::error!("{}", e);
                Err(ApiError::DbError)
            }
        }
    }

    pub async fn get_for_mod_versions(
        ids: &Vec<i32>,
        pool: &mut PgConnection,
    ) -> Result<HashMap<i32, Vec<FetchedIncompatibility>>, ApiError> {
        let result = match sqlx::query_as!(
            FetchedIncompatibility,
            r#"SELECT icp.compare as "compare: _",
            icp.importance as "importance: _",
            icp.incompatibility_id, icp.mod_id, icp.version FROM incompatibilities icp
            INNER JOIN mod_versions mv ON mv.id = icp.mod_id
            WHERE mv.id = ANY($1)"#,
            &ids,
        )
        .fetch_all(&mut *pool)
        .await
        {
            Ok(d) => d,
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
        };
        let mut ret: HashMap<i32, Vec<FetchedIncompatibility>> = HashMap::new();

        for i in result {
            ret.entry(i.mod_id).or_default().push(i);
        }

        Ok(ret)
    }
}
