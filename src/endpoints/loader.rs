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
			loader_version::{
				GetVersionsQuery, LoaderVersion, LoaderVersionCreate
			},
			mod_gd_version::{
				DetailedGDVersion,
				GDVersionEnum,
				VerPlatform,
			}
		}
	},
	config::AppData,
};

#[derive(Deserialize)]
struct GetOneQuery {
	platform: Option<VerPlatform>,
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
	let mut pool = data.db().acquire().await.or(Err(ApiError::DbAcquireError))?;

	let version = if path.version == "latest" {
		let gd = if let Some(i) = &query.gd {
			if let Ok(g) = GDVersionEnum::from_str(i) {
				Some(g)
			} else {
				let platform = query.platform
					.ok_or_else(|| ApiError::BadRequest("Platform is required when a version alias is given".to_string()))?;
				Some(GDVersionAlias::find(platform, i, &mut pool).await?)
			}
		} else {
			None
		};

		LoaderVersion::get_latest(gd, query.platform, query.prerelease, &mut pool).await?
	} else {
		LoaderVersion::get_one(&path.version, &mut pool).await?
	};

	Ok(web::Json(ApiResponse {
		error: "".to_string(),
		payload: version,
	}))
}

#[derive(Deserialize)]
struct CreateVersionBody {
	pub tag: String,
	#[serde(default)]
	pub prerelease: bool,
	pub commit_hash: String,
	pub gd: DetailedGDVersion,
}

#[post("v1/loader/versions")]
pub async fn create_version(
	data: web::Data<AppData>,
	payload: web::Json<CreateVersionBody>,
	auth: Auth,
) -> Result<impl Responder, ApiError> {
	let dev = auth.developer()?;
	let mut pool = data.db().acquire().await.or(Err(ApiError::DbAcquireError))?;

	if !dev.admin {
		return Err(ApiError::Forbidden);
	}

	let mut transaction = pool.begin().await.or(Err(ApiError::TransactionError))?;
	if let Err(e) = LoaderVersion::create_version(LoaderVersionCreate {
		tag: payload.tag.trim_start_matches('v').to_string(),
		prerelease: payload.prerelease,
		commit_hash: payload.commit_hash.clone(),
		win: payload.gd.win.clone(),
		mac: payload.gd.mac.clone(),
		android: payload.gd.android.clone(),
		ios: payload.gd.ios.clone(),
	}, &mut transaction).await {
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

#[derive(Deserialize)]
struct GetManyQuery {
	pub gd: Option<GDVersionEnum>,
	pub platform: Option<VerPlatform>,
	pub per_page: Option<i64>,
	pub page: Option<i64>,
	pub prerelease: Option<bool>
}

#[get("v1/loader/versions")]
pub async fn get_many(
	data: web::Data<AppData>,
	query: web::Query<GetManyQuery>,
) -> Result<impl Responder, ApiError> {
	let mut pool = data.db().acquire().await.or(Err(ApiError::DbAcquireError))?;

	let versions = LoaderVersion::get_many(
		GetVersionsQuery {
			gd: query.gd,
			platform: query.platform,
			prerelease: query.prerelease.unwrap_or_default()
		},
		query.per_page.unwrap_or(10),
		query.page.unwrap_or(1),
		&mut pool
	).await?;

	Ok(web::Json(ApiResponse {
		error: "".to_string(),
		payload: versions,
	}))
}
