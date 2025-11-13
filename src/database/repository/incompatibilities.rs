use sqlx::PgConnection;

use crate::{
    database::DatabaseError,
    types::{
        mod_json::ModJson,
        models::{
            dependency::ModVersionCompare,
            incompatibility::{FetchedIncompatibility, IncompatibilityImportance},
        },
    },
};

pub async fn create(
    mod_version_id: i32,
    json: &ModJson,
    conn: &mut PgConnection,
) -> Result<Vec<FetchedIncompatibility>, DatabaseError> {
    let incompats = json.prepare_incompatibilities_for_create().map_err(|e| {
        DatabaseError::InvalidInput(format!(
            "Failed to parse incompatibilities from mod.json: {e}"
        ))
    })?;
    if incompats.is_empty() {
        return Ok(vec![]);
    }

    let len = incompats.len();
    let mod_id = vec![mod_version_id; len];
    let mut incompatibility_id: Vec<String> = Vec::with_capacity(len);
    let mut version: Vec<String> = Vec::with_capacity(len);
    let mut compare: Vec<ModVersionCompare> = Vec::with_capacity(len);
    let mut importance: Vec<IncompatibilityImportance> = Vec::with_capacity(len);

    for i in incompats {
        incompatibility_id.push(i.incompatibility_id);
        version.push(i.version);
        compare.push(i.compare);
        importance.push(i.importance);
    }

    sqlx::query_as!(
        FetchedIncompatibility,
        r#"INSERT INTO incompatibilities
        (mod_id, incompatibility_id, version, compare, importance)
        SELECT * FROM UNNEST(
            $1::int4[],
            $2::text[],
            $3::text[],
            $4::version_compare[],
            $5::incompatibility_importance[]
        )
        RETURNING 
            mod_id,
            incompatibility_id,
            version,
            compare as "compare: _",
            importance as "importance: _""#,
        &mod_id,
        &incompatibility_id,
        &version,
        &compare as &[ModVersionCompare],
        &importance as &[IncompatibilityImportance]
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| log::error!("incompatibilities::create query failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn clear(id: i32, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM incompatibilities
            WHERE mod_id = $1",
        id
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("incompatibilities::clear query failed: {e}"))
    .map_err(|e| e.into())
    .map(|_| ())
}
