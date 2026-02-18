use crate::database::DatabaseError;
use crate::types::models::deprecations::Deprecation;
use sqlx::PgConnection;

pub async fn get_for_mods(
    ids: &[String],
    conn: &mut PgConnection,
) -> Result<Vec<Deprecation>, DatabaseError> {
    let deps = sqlx::query!(
        "SELECT d.id, d.mod_id, d.reason
            FROM deprecations d
        WHERE d.mod_id = ANY($1)",
        ids
    )
    .fetch_all(&mut *conn)
    .await
    .inspect_err(|e| log::error!("deprecations::get_for_mods failed: {e}"))?;

    let mut bys: Vec<_> = sqlx::query!(
        "SELECT dby.deprecation_id, dby.by_mod_id
            FROM deprecated_by dby
        WHERE dby.deprecation_id = ANY($1)",
        &deps.iter().map(|d| d.id).collect::<Vec<i32>>()
    )
    .fetch_all(&mut *conn)
    .await
    .inspect_err(|e| log::error!("deprecations::get_for_mods failed: {e}"))?;

    Ok(deps
        .into_iter()
        .map(|dep| Deprecation {
            id: dep.id,
            mod_id: dep.mod_id,
            by: bys
                .extract_if(.., |by| by.deprecation_id == dep.id)
                .map(|by| by.by_mod_id)
                .collect(),
            reason: dep.reason,
        })
        .collect())
}

pub async fn get(id: i32, conn: &mut PgConnection) -> Result<Option<Deprecation>, DatabaseError> {
    let dep = sqlx::query!(
        "SELECT
            id,
            mod_id,
            reason
        FROM deprecations
        WHERE id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await
    .inspect_err(|e| log::error!("deprecations::get failed: {e}"))?
    .map(|x| Deprecation {
        id,
        mod_id: x.mod_id,
        by: vec![],
        reason: x.reason,
    });

    if dep.is_none() {
        return Ok(None);
    }

    let mut dep = dep.unwrap();

    let deprecated_by = sqlx::query!(
        "SELECT by_mod_id
        FROM deprecated_by
        WHERE deprecation_id = $1",
        dep.id
    )
    .fetch_all(&mut *conn)
    .await
    .inspect_err(|e| log::error!("deprecations::get failed: {e}"))?;

    dep.by = deprecated_by.into_iter().map(|b| b.by_mod_id).collect();

    Ok(Some(dep))
}

pub async fn create(
    mod_id: &str,
    by: &[String],
    reason: &str,
    conn: &mut PgConnection,
) -> Result<Deprecation, DatabaseError> {
    let id = sqlx::query!(
        "INSERT INTO deprecations(mod_id, reason)
            VALUES ($1, $2)
            RETURNING id",
        mod_id,
        reason
    )
    .fetch_one(&mut *conn)
    .await
    .inspect_err(|e| log::error!("deprecations::create failed: {e}"))?
    .id;

    if !by.is_empty() {
        insert_deprecated_by(id, by, &mut *conn).await?;
    }

    Ok(Deprecation {
        id,
        mod_id: mod_id.to_string(),
        by: by.to_vec(),
        reason: reason.to_string(),
    })
}

pub async fn update(
    mut deprecation: Deprecation,
    by: Option<&[String]>,
    reason: Option<&str>,
    conn: &mut PgConnection,
) -> Result<Deprecation, DatabaseError> {
    if by.is_none() && reason.is_none() {
        return Ok(deprecation);
    }

    let mut updated_timestamp = false;

    if let Some(reason) = reason {
        sqlx::query!(
            "UPDATE deprecations SET
                reason = $1,
                updated_at = NOW()
            WHERE id = $2",
            reason,
            deprecation.id
        )
        .execute(&mut *conn)
        .await
        .inspect_err(|e| log::error!("deprecations::update failed: {e}"))?;

        deprecation.reason = reason.to_string();

        updated_timestamp = true;
    }

    if let Some(by) = by {
        sqlx::query!(
            "DELETE FROM deprecated_by
            WHERE deprecation_id = $1",
            deprecation.id
        )
        .execute(&mut *conn)
        .await
        .inspect_err(|e| log::error!("deprecations::update failed: {e}"))?;

        insert_deprecated_by(deprecation.id, by, &mut *conn).await?;

        deprecation.by = by.to_vec();
    }

    if !updated_timestamp {
        sqlx::query!(
            "UPDATE deprecations SET
                updated_at = NOW()
            WHERE id = $1",
            deprecation.id
        )
        .execute(&mut *conn)
        .await?;
    }

    Ok(deprecation)
}

pub async fn delete(id: i32, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM deprecations
        WHERE id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| log::error!("deprecations::delete failed: {e}"))?;

    Ok(())
}

pub async fn clear_all(mod_id: &str, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM deprecations
        WHERE mod_id = $1",
        mod_id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| log::error!("deprecations::clear_all failed: {e}"))?;

    Ok(())
}

async fn insert_deprecated_by(
    id: i32,
    by: &[String],
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    if by.is_empty() {
        return Ok(());
    }

    let deprecation_id = vec![id; by.len()];

    sqlx::query!(
        "INSERT INTO deprecated_by
            (deprecation_id, by_mod_id)
            SELECT * FROM UNNEST(
                $1::int4[],
                $2::text[]
            )",
        &deprecation_id,
        by
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| log::error!("deprecations::insert_deprecated_by failed: {e}"))
    .map(|_| ())
    .map_err(|e| e.into())
}
