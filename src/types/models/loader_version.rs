use crate::types::{
	models::mod_gd_version::{GDVersionEnum, VerPlatform},
	api::ApiError,
};

use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};

use sqlx::{
	types::chrono::{DateTime, Utc},
	PgConnection, Postgres, QueryBuilder
};

#[derive(sqlx::FromRow, Deserialize, Serialize, Debug, Clone)]
pub struct LoaderGDVersion {
	pub win: Option<GDVersionEnum>,
	pub android: Option<GDVersionEnum>,
	pub mac: Option<GDVersionEnum>,
}

#[derive(Debug)]
pub struct LoaderVersionCreate {
	pub tag: String,
	pub gd: LoaderGDVersion,
	pub prerelease: bool
}

#[derive(Serialize, Debug)]
pub struct LoaderVersion {
	pub tag: String,
	pub gd: LoaderGDVersion,
	pub prerelease: bool,
	pub created_at: String,
}

#[derive(sqlx::FromRow, Debug)]
pub struct LoaderVersionGetOne {
	pub tag: String,
	#[sqlx(flatten)]
	pub gd: LoaderGDVersion,
	pub prerelease: bool,
	pub created_at: DateTime<Utc>
}

pub struct GetVersionsQuery {
	pub gd: Option<GDVersionEnum>,
	pub platform: Option<VerPlatform>,
	pub prerelease: bool
}

impl LoaderVersionGetOne {
	pub fn into_loader_version(self) -> LoaderVersion {
		LoaderVersion {
			tag: self.tag,
			prerelease: self.prerelease,
			created_at: self.created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
			gd: self.gd
		}
	}
}

impl LoaderVersion {
	pub async fn get_latest(
		gd: Option<GDVersionEnum>,
		platform: Option<VerPlatform>,
		accept_prereleases: bool,
		pool: &mut PgConnection,
	) -> Result<LoaderVersion, ApiError> {
		let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
			r#"SELECT
				mac, win, android, tag, created_at, prerelease
			FROM geode_versions"#
		);

		match (platform, gd) {
			(Some(p), Some(g)) => {
				match p {
					VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => query_builder.push(" WHERE android="),
					VerPlatform::Mac | VerPlatform::MacIntel | VerPlatform::MacArm => query_builder.push(" WHERE mac="),
					VerPlatform::Win => query_builder.push(" WHERE win="),
					_ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
				};

				query_builder.push_bind(g);
			}
			(Some(_), None) => {
				// this option will be handled later by ordering tricks
				query_builder.push(" WHERE 1=1");
			}
			(None, Some(g)) => {
				query_builder.push(" WHERE android=");
				query_builder.push_bind(g);
				query_builder.push(" or mac=");
				query_builder.push_bind(g);
				query_builder.push(" or win=");
				query_builder.push_bind(g);
			}
			(None, None) => {
			// if gd version isn't specifed, select whatever versions have the latest gd version
			query_builder.push(
				r#" WHERE
					android=enum_last(NULL::gd_version) OR
					win=enum_last(NULL::gd_version) OR
					mac=enum_last(NULL::gd_version)
				"#);
			}
		}

		if !accept_prereleases {
			query_builder.push(" AND prerelease=FALSE ");
		}

		query_builder.push(" ORDER BY ");

		if gd.is_none() {
			if let Some(p) = platform {
				// if there's a platform but no gd, order by the latest gd for that platform
				match p {
					VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => query_builder.push(" android"),
					VerPlatform::Mac | VerPlatform::MacIntel | VerPlatform::MacArm => query_builder.push(" mac"),
					VerPlatform::Win => query_builder.push(" win"),
					_ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
				};
				query_builder.push(" DESC, ");
			}
		}

		query_builder.push(" created_at DESC LIMIT 1;");

		match query_builder
			.build_query_as::<LoaderVersionGetOne>()
			.fetch_optional(&mut *pool)
			.await
		{
			Ok(Some(r)) => Ok(r.into_loader_version()),
			Ok(None) => Err(ApiError::NotFound("".to_string())),
			Err(e) => {
					log::error!("{:?}", e);
					Err(ApiError::DbError)
			}
		}
	}

	pub async fn get_one(tag: &str, pool: &mut PgConnection) -> Result<LoaderVersion, ApiError> {
		match sqlx::query_as::<Postgres, LoaderVersionGetOne>(
			r#"SELECT
				mac, win, android, tag, created_at, prerelease
			FROM geode_versions
				WHERE tag = $1"#)
			.bind(tag)
			.fetch_optional(&mut *pool)
			.await
		{
			Ok(Some(r)) => Ok(r.into_loader_version()),
			Ok(None) => Err(ApiError::NotFound("".to_string())),
			Err(e) => {
					log::error!("{:?}", e);
					Err(ApiError::DbError)
			}
		}
	}

	pub async fn create_version(version: LoaderVersionCreate, pool: &mut PgConnection) -> Result<(), ApiError> {
		match sqlx::query(
			r#"INSERT INTO geode_versions
				(tag, prerelease, mac, win, android)
			VALUES
				($1, $2, $3, $4, $5)"#)
			.bind(version.tag)
			.bind(version.prerelease)
			.bind(version.gd.mac)
			.bind(version.gd.win)
			.bind(version.gd.android)
			.execute(&mut *pool)
			.await
		{
			Ok(_) => Ok(()),
			Err(e) => {
					log::error!("{:?}", e);
					Err(ApiError::DbError)
			}
		}
	}

	pub async fn get_many(
		query: GetVersionsQuery,
		per_page: i64,
		page: i64,
		pool: &mut PgConnection
	) -> Result<Vec<LoaderVersion>, ApiError> {
		let limit = per_page;
		let offset = (page - 1) * per_page;

		let mut query_builder = QueryBuilder::new(r#"
			SELECT mac, win, android, tag, created_at, prerelease FROM geode_versions
		"#);

		match (query.platform, query.gd) {
			(Some(p), Some(g)) => {
				match p {
					VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => query_builder.push(" WHERE android="),
					VerPlatform::Mac | VerPlatform::MacIntel | VerPlatform::MacArm => query_builder.push(" WHERE mac="),
					VerPlatform::Win => query_builder.push(" WHERE win="),
					_ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
				};

				query_builder.push_bind(g);
			}
			(Some(p), None) => {
				match p {
					VerPlatform::Android | VerPlatform::Android32 | VerPlatform::Android64 => query_builder.push(" WHERE android IS NOT NULL"),
					VerPlatform::Mac | VerPlatform::MacIntel | VerPlatform::MacArm => query_builder.push(" WHERE mac IS NOT NULL"),
					VerPlatform::Win => query_builder.push(" WHERE win IS NOT NULL"),
					_ => return Err(ApiError::BadRequest("Invalid platform".to_string())),
				};
			}
			(None, Some(g)) => {
				query_builder.push(" WHERE android=");
				query_builder.push_bind(g);
				query_builder.push(" or mac=");
				query_builder.push_bind(g);
				query_builder.push(" or win=");
				query_builder.push_bind(g);
			}
			_ => {
				query_builder.push(" WHERE 1=1");
			}
		}

		if !query.prerelease {
			query_builder.push(" AND prerelease=FALSE ");
		}

		query_builder.push(" ORDER BY created_at DESC ");

		query_builder.push(" LIMIT ");
		query_builder.push_bind(limit);
		query_builder.push(" OFFSET ");
		query_builder.push_bind(offset);

		match query_builder
			.build_query_as::<LoaderVersionGetOne>()
			.fetch_all(&mut *pool)
			.await
		{
			Ok(r) =>
				Ok(r.into_iter().map(|x| x.into_loader_version()).collect()),
			Err(e) => {
					log::error!("{:?}", e);
					Err(ApiError::DbError)
			}
		}
	}
}
