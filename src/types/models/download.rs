use sqlx::types::ipnetwork::IpNetwork;
use sqlx::PgConnection;

use crate::types::api::ApiError;

pub async fn create_download(
	ip: IpNetwork,
	mod_version_id: i32,
	pool: &mut PgConnection,
) -> Result<bool, ApiError> {
	let existing = match sqlx::query!(
		r#"
        SELECT * FROM mod_downloads
        WHERE ip = $1 AND mod_version_id = $2
        "#,
		ip,
		mod_version_id
	)
	.fetch_optional(&mut *pool)
	.await
	{
		Ok(e) => e,
		Err(e) => {
			log::error!("{}", e);
			return Err(ApiError::InternalError);
		}
	};

	if existing.is_some() {
		return Ok(false);
	}

	match sqlx::query!(
		r#"
        INSERT INTO mod_downloads (ip, mod_version_id)
        VALUES ($1, $2)
        "#,
		ip,
		mod_version_id
	)
	.execute(&mut *pool)
	.await
	{
		Ok(_) => Ok(true),
		Err(e) => {
			log::error!("{}", e);
			Err(ApiError::InternalError)
		}
	}
}
