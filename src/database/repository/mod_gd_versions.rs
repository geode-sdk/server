use crate::types::api::ApiError;
use crate::types::models::mod_gd_version::{GDVersionEnum, ModGDVersionCreate, VerPlatform};
use sqlx::{PgConnection, Postgres, QueryBuilder};

pub async fn create_from_json(
    json: &[ModGDVersionCreate],
    mod_version_id: i32,
    conn: &mut PgConnection,
) -> Result<(), ApiError> {
    if json.is_empty() {
        return Err(ApiError::BadRequest(
            "mod.json has no gd versions added".into(),
        ));
    }

    let mut builder: QueryBuilder<Postgres> =
        QueryBuilder::new("INSERT INTO mod_gd_versions (gd, platform, mod_id) VALUES ");

    for (i, current) in json.iter().enumerate() {
        builder.push("(");
        let mut separated = builder.separated(", ");
        separated.push_bind(current.gd as GDVersionEnum);
        separated.push_bind(current.platform as VerPlatform);
        separated.push_bind(mod_version_id);
        separated.push_unseparated(")");
        if i != json.len() - 1 {
            separated.push_unseparated(", ");
        }
    }

    builder.build().execute(conn).await.map_err(|e| {
        log::error!("Failed to insert mod_gd_versions: {}", e);
        ApiError::DbError
    })?;

    Ok(())
}
