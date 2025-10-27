use sqlx::PgConnection;

use crate::{
    database::DatabaseError,
    types::{
        mod_json::ModJson,
        models::{
            dependency::{DependencyImportance, FetchedDependency, ModVersionCompare},
            mod_gd_version::VerPlatform,
        },
    },
};

pub async fn create(
    mod_version_id: i32,
    json: &ModJson,
    conn: &mut PgConnection,
) -> Result<Vec<FetchedDependency>, DatabaseError> {
    let dependencies = json.prepare_dependencies_for_create().map_err(|e| {
        DatabaseError::InvalidInput(format!("Failed to parse dependencies from mod.json: {e}"))
    })?;
    if dependencies.is_empty() {
        return Ok(vec![]);
    }

    let len = dependencies.len();
    let dependent_id = vec![mod_version_id; len];
    let mut dependency_id: Vec<String> = Vec::with_capacity(len);
    let mut version: Vec<String> = Vec::with_capacity(len);
    let mut compare: Vec<ModVersionCompare> = Vec::with_capacity(len);
    let mut importance: Vec<DependencyImportance> = Vec::with_capacity(len);
    let mut windows: Vec<bool> = Vec::with_capacity(len);
    let mut mac_intel: Vec<bool> = Vec::with_capacity(len);
    let mut mac_arm: Vec<bool> = Vec::with_capacity(len);
    let mut android32: Vec<bool> = Vec::with_capacity(len);
    let mut android64: Vec<bool> = Vec::with_capacity(len);
    let mut ios: Vec<bool> = Vec::with_capacity(len);

    for i in dependencies {
        dependency_id.push(i.dependency_id);
        version.push(i.version);
        compare.push(i.compare);
        importance.push(i.importance);

        windows.push(
            i.platforms
                .as_ref()
                .is_none_or(|x| x.contains(&VerPlatform::Win)),
        );
        mac_intel.push(
            i.platforms
                .as_ref()
                .is_none_or(|x| x.contains(&VerPlatform::MacIntel)),
        );
        mac_arm.push(
            i.platforms
                .as_ref()
                .is_none_or(|x| x.contains(&VerPlatform::MacArm)),
        );
        android32.push(
            i.platforms
                .as_ref()
                .is_none_or(|x| x.contains(&VerPlatform::Android32)),
        );
        android64.push(
            i.platforms
                .as_ref()
                .is_none_or(|x| x.contains(&VerPlatform::Android64)),
        );
        ios.push(
            i.platforms
                .as_ref()
                .is_none_or(|x| x.contains(&VerPlatform::Ios)),
        );
    }

    sqlx::query_as!(
        FetchedDependency,
        r#"INSERT INTO dependencies 
        (dependent_id, dependency_id, version,
        compare, importance, windows, mac_intel, mac_arm,
        android32, android64, ios)
        SELECT * FROM UNNEST(
            $1::int4[],
            $2::text[],
            $3::text[],
            $4::version_compare[],
            $5::dependency_importance[],
            $6::bool[],
            $7::bool[],
            $8::bool[],
            $9::bool[],
            $10::bool[],
            $11::bool[]
        )
        RETURNING 
            dependent_id as mod_version_id,
            dependency_id,
            version,
            compare as "compare: _",
            importance as "importance: _",
            windows,
            mac_intel,
            mac_arm,
            android32,
            android64,
            ios"#,
        &dependent_id,
        &dependency_id,
        &version,
        &compare as &[ModVersionCompare],
        &importance as &[DependencyImportance],
        &windows,
        &mac_intel,
        &mac_arm,
        &android32,
        &android64,
        &ios
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| log::error!("dependenceis::create query failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn clear(id: i32, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM dependencies
            WHERE dependent_id = $1",
        id
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("dependencies::clear query failed: {e}"))
    .map_err(|e| e.into())
    .map(|_| ())
}
