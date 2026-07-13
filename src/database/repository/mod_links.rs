use sqlx::PgConnection;

use crate::{database::DatabaseError, types::models::mod_link::ModLinks};

#[tracing::instrument(skip_all, fields(mod_id = %mod_id))]
pub async fn upsert(
    mod_id: &str,
    community: Option<String>,
    homepage: Option<String>,
    source: Option<String>,
    conn: &mut PgConnection,
) -> Result<ModLinks, DatabaseError> {
    sqlx::query!(
        "INSERT INTO mod_links
            (mod_id, community, homepage, source)
        VALUES
            ($1, $2, $3, $4)
        ON CONFLICT (mod_id)
        DO UPDATE SET
            community = $2,
            homepage = $3,
            source = $4",
        mod_id,
        community,
        homepage,
        source
    )
    .execute(&mut *conn)
    .await
    .inspect_err(|e| tracing::error!("{:?}", e))?;

    Ok(ModLinks {
        mod_id: mod_id.into(),
        community,
        homepage,
        source,
    })
}
