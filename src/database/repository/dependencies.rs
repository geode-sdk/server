use sqlx::PgConnection;

use crate::types::{
    api::ApiError,
    mod_json::ModJson,
    models::dependency::{DependencyImportance, FetchedDependency, ModVersionCompare},
};

pub async fn create(
    mod_version_id: i32,
    json: &ModJson,
    conn: &mut PgConnection,
) -> Result<Vec<FetchedDependency>, ApiError> {
    let dependencies = json.prepare_dependencies_for_create()?;
    if dependencies.is_empty() {
        return Ok(vec![]);
    }

    let len = dependencies.len();
    let dependent_id = vec![mod_version_id; len];
    let mut dependency_id: Vec<String> = Vec::with_capacity(len);
    let mut version: Vec<String> = Vec::with_capacity(len);
    let mut compare: Vec<ModVersionCompare> = Vec::with_capacity(len);
    let mut importance: Vec<DependencyImportance> = Vec::with_capacity(len);

    for i in dependencies {
        dependency_id.push(i.dependency_id);
        version.push(i.version);
        compare.push(i.compare);
        importance.push(i.importance);
    }

    sqlx::query_as!(
        FetchedDependency,
        r#"INSERT INTO dependencies 
        (dependent_id, dependency_id, version, compare, importance)
        SELECT * FROM UNNEST(
            $1::int4[],
            $2::text[],
            $3::text[],
            $4::version_compare[],
            $5::dependency_importance[]
        )
        RETURNING 
            dependent_id as mod_version_id,
            dependency_id,
            version,
            compare as "compare: _",
            importance as "importance: _""#,
        &dependent_id,
        &dependency_id,
        &version,
        &compare as &[ModVersionCompare],
        &importance as &[DependencyImportance]
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| log::error!("Failed to insert dependencies: {}", e))
    .or(Err(ApiError::DbError))
}

pub async fn clear(id: i32, conn: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!(
        "DELETE FROM dependencies
            WHERE dependent_id = $1",
        id
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("Failed to clear deps: {}", e))
    .or(Err(ApiError::DbError))?;

    Ok(())
}
