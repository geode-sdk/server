use serde::Serialize;
use sqlx::PgConnection;

use crate::endpoints::ApiError;

#[derive(sqlx::FromRow, Serialize)]
pub struct Deprecation {
    pub mod_id: String,
    pub by: Vec<String>,
    pub reason: String,
}

impl Deprecation {
    pub async fn get_deprecations_for(pool: &mut PgConnection, ids: &[String])
        -> Result<Vec<Deprecation>, ApiError>
    {
        let deps = sqlx::query!(
            r#"
            SELECT d.id, d.mod_id, d.reason
            FROM deprecations d
            WHERE d.mod_id = ANY($1)
            "#,
            ids
        ).fetch_all(&mut *pool).await?;

        let mut bys: Vec<_> = sqlx::query!(
            r#"
            SELECT dby.deprecation_id, dby.by_mod_id
            FROM deprecated_by dby
            WHERE dby.deprecation_id = ANY($1)
            "#,
            &deps.iter().map(|d| d.id).collect::<Vec<i32>>()
        ).fetch_all(&mut *pool).await?;

        Ok(deps.into_iter().map(|dep| Deprecation {
            mod_id: dep.mod_id,
            by: bys
                .extract_if(.., |by| by.deprecation_id == dep.id)
                .map(|by| by.by_mod_id)
                .collect(),
            reason: dep.reason,
        }).collect())
    }

    pub async fn create_deprecation(pool: &mut PgConnection, mod_id: &str, by: &[String], reason: &str)
        -> Result<(), ApiError>
    {
        let id = sqlx::query!(
            r#"
            INSERT INTO deprecations(mod_id, reason)
            VALUES ($1, $2)
            RETURNING id
            "#,
            mod_id, reason
        ).fetch_one(&mut *pool).await?.id;
        
        // I'm not sure how you'd insert multiple values at once with sqlx but 
        // this endpoint shouldn't be called very often so this oughta be fine
        for by_id in by {
            sqlx::query!(
                r#"
                INSERT INTO deprecated_by(deprecation_id, by_mod_id)
                VALUES ($1, $2)
                "#,
                id, by_id
            ).execute(&mut *pool).await?;
        }

        Ok(())
    }
    pub async fn delete_deprecation(pool: &mut PgConnection, mod_id: &str)
        -> Result<(), ApiError>
    {
        sqlx::query!(
            r#"
            DELETE FROM deprecations
            WHERE mod_id = $1
            "#,
            mod_id
        ).execute(&mut *pool).await?;
        Ok(())
    }
}
