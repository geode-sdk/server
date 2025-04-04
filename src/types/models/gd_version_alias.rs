use crate::types::{
	models::mod_gd_version::{GDVersionEnum, VerPlatform},
	api::ApiError,
};

use chrono::SecondsFormat;
use serde::Serialize;

use sqlx::{
	types::{
		chrono::{DateTime, Utc},
		Uuid
	}, PgConnection, Postgres, QueryBuilder
};

#[derive(Serialize)]
pub struct GDVersionAlias {
	pub version_name: GDVersionEnum,
	pub mac_arm_uuid: Option<String>,
	pub mac_intel_uuid: Option<String>,
	pub android_manifest_id: Option<i32>,
	pub windows_timestamp: Option<i32>,
	pub ios_bundle_version: Option<String>,
	pub added_at: String
}

impl GDVersionAlias {
	pub async fn find(
		platform: VerPlatform,
		identifier: &str,
		pool: &mut PgConnection,
	) -> Result<GDVersionEnum, ApiError>  {
		let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
			r#"SELECT version_name FROM gd_version_aliases"#
		);

		match platform {
			VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => {
				let manifest_id = identifier.parse::<i32>()
					.map_err(|_| ApiError::BadRequest("Identifier is not a valid manifest id".to_string()))?;

				query_builder.push(" WHERE android_manifest_id=");
				query_builder.push_bind(manifest_id);
			},
			VerPlatform::Mac => {
				let uuid = Uuid::parse_str(identifier)
					.map_err(|_| ApiError::BadRequest("Identifier is not a valid UUID".to_string()))?;

				query_builder.push(" WHERE mac_arm_uuid=");
				query_builder.push_bind(uuid);
				query_builder.push(" OR mac_intel_uuid=");
				query_builder.push_bind(uuid);
			},
			VerPlatform::MacArm => {
				let uuid = Uuid::parse_str(identifier)
					.map_err(|_| ApiError::BadRequest("Identifier is not a valid UUID".to_string()))?;

				query_builder.push(" WHERE mac_arm_uuid=");
				query_builder.push_bind(uuid);
			},
			VerPlatform::MacIntel => {
				let uuid = Uuid::parse_str(identifier)
					.map_err(|_| ApiError::BadRequest("Identifier is not a valid UUID".to_string()))?;

				query_builder.push(" WHERE mac_intel_uuid=");
				query_builder.push_bind(uuid);
			},
			VerPlatform::Win => {
				let timestamp = identifier.parse::<i32>()
					.map_err(|_| ApiError::BadRequest("Identifier is not a valid timestamp".to_string()))?;

				query_builder.push(" WHERE windows_timestamp=");
				query_builder.push_bind(timestamp);
			},
			VerPlatform::Ios => {
				let bundle_version = identifier.to_string();
				query_builder.push(" WHERE ios_bundle_version=");
				query_builder.push_bind(bundle_version);
			}
			// _ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
		};

		// probably useless?
		query_builder.push(" ORDER BY added_at DESC LIMIT 1");

		match query_builder
			.build_query_scalar::<GDVersionEnum>()
			.fetch_optional(&mut *pool)
			.await
		{
			Ok(Some(v)) => Ok(v),
			Ok(None) => Err(ApiError::NotFound("".to_string())),
			Err(e) => {
					log::error!("{:?}", e);
					Err(ApiError::DbError)
			}
		}
	}
}
