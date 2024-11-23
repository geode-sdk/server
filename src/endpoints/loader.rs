use std::str::FromStr;
use actix_web::{web, get, Responder};
use serde::Deserialize;

use crate::{
	types::{
		api::{ApiError, ApiResponse},
		models::{
			loader_version::LoaderVersion,
			mod_gd_version::{GDVersionEnum, VerPlatform}
		}
	},
	AppData,
};

#[derive(Deserialize)]
struct GetOneQuery {
	platform: Option<String>,
	gd: Option<String>,
	#[serde(default)]
	prerelease: bool,
}

#[derive(Deserialize)]
struct GetOnePath {
	version: String,
}

#[get("v1/loader/versions/{version}")]
pub async fn get_one(
	path: web::Path<GetOnePath>,
	data: web::Data<AppData>,
	query: web::Query<GetOneQuery>
) -> Result<impl Responder, ApiError> {
	let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

	let version = if path.version == "latest" {
		let gd = query.gd.as_ref()
			.map(|s| GDVersionEnum::from_str(s))
			.transpose()
			.map_err(|_| ApiError::BadRequest("Invalid gd".to_string()))?;

		let platform = query.platform.as_ref()
			.map(|s| VerPlatform::from_str(s))
			.transpose()
			.map_err(|_| ApiError::BadRequest("Invalid platform".to_string()))?;

		LoaderVersion::get_latest(gd, platform, query.prerelease, &mut pool).await?
	} else {
		LoaderVersion::get_one(&path.version, &mut pool).await?
	};

	Ok(web::Json(ApiResponse {
			error: "".to_string(),
			payload: version,
	}))
}
