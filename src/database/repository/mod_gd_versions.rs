use sqlx::PgConnection;

use crate::{
    database::DatabaseError,
    types::{
        mod_json::ModJson,
        models::mod_gd_version::{DetailedGDVersion, GDVersionEnum, VerPlatform},
    },
};

pub async fn create(
    mod_version_id: i32,
    json: &ModJson,
    conn: &mut PgConnection,
) -> Result<DetailedGDVersion, DatabaseError> {
    let create = json.gd.to_create_payload(json);

    let gd: Vec<GDVersionEnum> = create.iter().map(|x| x.gd).collect();
    let platform: Vec<VerPlatform> = create.iter().map(|x| x.platform).collect();
    let mod_id = vec![mod_version_id; create.len()];

    sqlx::query!(
        "INSERT INTO mod_gd_versions
        (gd, platform, mod_id)
        SELECT * FROM UNNEST(
            $1::gd_version[],
            $2::gd_ver_platform[],
            $3::int4[]
        )",
        &gd as &[GDVersionEnum],
        &platform as &[VerPlatform],
        &mod_id
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("mod_gd_versions::create query failed: {e}"))?;

    Ok(json.gd.clone())
}

pub async fn clear(mod_version_id: i32, conn: &mut PgConnection) -> Result<(), DatabaseError> {
    sqlx::query!(
        "DELETE FROM mod_gd_versions mgv
            WHERE mgv.mod_id = $1",
        mod_version_id
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| log::error!("incompatibilities::clear query failed: {e}"))
    .map_err(|e| e.into())
    .map(|_| ())
}
