use semver::Version;
use sqlx::PgConnection;

use crate::types::{api::ApiError, models::mod_version::ModVersion};

use super::developer::FetchedDeveloper;

pub async fn reject_mod(
    id: &str,
    version: Version,
    reason: Option<String>,
    admin: &FetchedDeveloper,
    pool: &mut PgConnection,
) -> Result<(), ApiError> {
    if !admin.admin {
        return Err(ApiError::Forbidden);
    }

    struct FoundRecord {
        mod_id: String,
        id: i32,
    }

    let exists: FoundRecord = match sqlx::query_as!(
        FoundRecord,
        "select mod_id, id from mod_versions 
        where mod_id = $1 and version = $2 and validated = false",
        id,
        version.to_string()
    )
    .fetch_optional(&mut *pool)
    .await
    {
        Ok(Some(e)) => e,
        Ok(None) => return Err(ApiError::NotFound("".to_string())),
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
    };

    let rejection_exists = match sqlx::query!(
        "select exists(select 1 from mod_rejections where mod_id = $1 and version = $2)",
        exists.mod_id,
        version.to_string()
    )
    .fetch_one(&mut *pool)
    .await
    {
        Ok(e) => e.exists.unwrap_or(false),
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
    };

    if rejection_exists {
        return Err(ApiError::BadRequest(
            "Mod version has already been rejected once".to_string(),
        ));
    }

    if let Err(e) = sqlx::query!(
        "insert into mod_rejections (mod_id, version, reason, admin_id) values ($1, $2, $3, $4)",
        exists.mod_id,
        version.to_string(),
        reason,
        admin.id
    )
    .execute(&mut *pool)
    .await
    {
        log::error!("{}", e);
        return Err(ApiError::DbError);
    }

    ModVersion::delete_version(exists.id, pool).await?;
    Ok(())
}

pub async fn remove_rejection(
    id: &str,
    version: Version,
    pool: &mut PgConnection,
) -> Result<(), ApiError> {
    let result = match sqlx::query!(
        "delete from mod_rejections where mod_id = $1 and version = $2",
        id,
        version.to_string()
    )
    .execute(&mut *pool)
    .await
    {
        Ok(e) => e,
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::DbError);
        }
    };

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("".to_string()));
    }

    Ok(())
}
