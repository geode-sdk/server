use sqlx::{types::ipnetwork::IpNetwork, PgConnection, Acquire};

use crate::types::api::ApiError;

pub async fn downloaded_version(mod_version_id: i32, pool: &mut PgConnection) -> Result<bool, ApiError> {
    match sqlx::query!(
        r#"
        SELECT mod_version_id FROM mod_downloads md
        WHERE mod_version_id = $1 LIMIT 1
        "#,
        mod_version_id
    )
    .fetch_optional(&mut *pool)
    .await
    {
        Ok(e) => Ok(e.is_some()),
        Err(e) => {
            log::error!("{}", e);
            Err(ApiError::InternalError)
        }
    }
}

pub async fn create_download(
    ip: IpNetwork,
    mod_version_id: i32,
    mod_id: &str,
    pool: &mut PgConnection,
) -> Result<(bool, bool), ApiError> {
    // hold it in a transaction, so we don't get duplicate downloads or something
    let mut tx = pool.begin().await.or(Err(ApiError::TransactionError))?;

    let existing = match sqlx::query!(
        r#"
        SELECT mod_version_id FROM mod_downloads md
        WHERE ip = $1 AND mod_version_id = $2 FOR UPDATE
        "#,
        ip,
        mod_version_id
    )
    .fetch_optional(&mut *tx)
    .await
    {
        Ok(e) => e,
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::InternalError);
        }
    };

    if existing.is_some() {
        // we don't really care about a read transaction failing
        tx.commit().await.or(Err(ApiError::TransactionError))?;

        return Ok((false, false));
    }

    // determine if the user has ever downloaded this mod
    // this is probably wasteful tbh

    let existing_mod = match sqlx::query!(
        r#"
        SELECT md.mod_version_id FROM mod_downloads md
        INNER JOIN mod_versions mv ON md.mod_version_id = mv.id
        INNER JOIN mod_version_statuses mvs ON mvs.mod_version_id = mv.id
        WHERE mv.mod_id = $2 AND mvs.status = 'accepted' AND ip = $1 LIMIT 1;
        "#,
        ip,
        mod_id
    )
    .fetch_optional(&mut *tx)
    .await
    {
        Ok(e) => e,
        Err(e) => {
            log::error!("{}", e);
            return Err(ApiError::InternalError);
        }
    };

    let downloaded_mod = existing_mod.is_some();

    match sqlx::query!(
        r#"
        INSERT INTO mod_downloads (ip, mod_version_id)
        VALUES ($1, $2)
        "#,
        ip,
        mod_version_id
    )
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {
            tx.commit().await.or(Err(ApiError::TransactionError))?;

            Ok((true, !downloaded_mod))
        },
        Err(e) => {
            log::error!("{}", e);
            Err(ApiError::InternalError)
        }
    }
}
