use std::str::FromStr;
use actix_web::{web, get, post, Responder, HttpResponse};
use serde::Deserialize;

use sqlx::Acquire;

use crate::{
	extractors::auth::Auth,
	types::{
		api::{ApiError, ApiResponse},
		models::{
			gd_version_alias::GDVersionAlias,
			loader_version::{LoaderVersion, LoaderVersionCreate},
			mod_gd_version::{GDVersionEnum, VerPlatform}
		}
	},
	AppData,
};

#[derive(Deserialize)]
struct GetOneQuery {
	platform: Option<String>,
	gd: Option<String>,
	identifier: Option<String>,
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
		let platform = query.platform.as_ref()
			.map(|s| VerPlatform::from_str(s))
			.transpose()
			.map_err(|_| ApiError::BadRequest("Invalid platform".to_string()))?;

		// my mess
		let gd = match (&query.gd, &query.identifier) {
			(Some(_), Some(_)) => Err(ApiError::BadRequest("Fields identifier and gd are mutually exclusive".to_string()))?,
			(Some(gd), None) => {
				Some(GDVersionEnum::from_str(gd)
					.map_err(|_| ApiError::BadRequest("Invalid gd".to_string()))?)
			}
			(None, Some(i)) => {
				let platform = platform
					.ok_or_else(|| ApiError::BadRequest("Field platform is required when a version identifier is provided".to_string()))?;
				Some(GDVersionAlias::find(platform, i, &mut pool).await?)
			},
			(None, None) => None
		};

		LoaderVersion::get_latest(gd, platform, query.prerelease, &mut pool).await?
	} else {
		LoaderVersion::get_one(&path.version, &mut pool).await?
	};

	Ok(web::Json(ApiResponse {
			error: "".to_string(),
			payload: version,
	}))
}

#[post("v1/loader/versions")]
pub async fn create_version(
	data: web::Data<AppData>,
	payload: web::Json<LoaderVersionCreate>,
	auth: Auth,
) -> Result<impl Responder, ApiError> {
	let dev = auth.developer()?;
	let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

	if !dev.admin {
		return Err(ApiError::Forbidden);
	}

	let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
	if let Err(e) = LoaderVersion::create_version(payload.into_inner(), &mut transaction).await {
		transaction
			.rollback()
			.await
			.or(Err(ApiError::TransactionError))?;
		return Err(e);
	}

	transaction
		.commit()
		.await
		.or(Err(ApiError::TransactionError))?;

	Ok(HttpResponse::NoContent())
}
