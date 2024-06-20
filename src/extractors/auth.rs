use std::pin::Pin;

use actix_web::{web, FromRequest, HttpRequest};
use futures::Future;
use uuid::Uuid;

use crate::types::api::ApiError;
use crate::types::models::developer::FetchedDeveloper;
use crate::AppData;

pub struct Auth {
	developer: Option<FetchedDeveloper>,
	token: Option<Uuid>,
}

impl Auth {
	/**
	 * Returns Ok(developer) if token was valid in request or returns ApiError::Unauthorized otherwise
	 */
	pub fn developer(&self) -> Result<FetchedDeveloper, ApiError> {
		match &self.developer {
			None => Err(ApiError::Unauthorized),
			Some(d) => Ok(d.clone()),
		}
	}

	pub fn token(&self) -> Result<Uuid, ApiError> {
		match self.token {
			None => Err(ApiError::Unauthorized),
			Some(t) => Ok(t),
		}
	}
}

impl FromRequest for Auth {
	type Error = ApiError;
	type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

	fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
		let data = req.app_data::<web::Data<AppData>>().unwrap().clone();
		let headers = req.headers().clone();
		Box::pin(async move {
			let token = match headers.get("Authorization") {
				None => {
					return Ok(Auth {
						developer: None,
						token: None,
					})
				}
				Some(t) => match t.to_str() {
					Err(e) => {
						log::error!("Failed to parse auth token: {}", e);
						return Ok(Auth {
							developer: None,
							token: None,
						});
					}
					Ok(str) => {
						let split = str.split(' ').collect::<Vec<&str>>();
						if split.len() != 2 || split[0] != "Bearer" {
							return Ok(Auth {
								developer: None,
								token: None,
							});
						}
						match Uuid::try_parse(split[1]) {
							Err(e) => {
								log::error!("Failed to parse auth token {}, error: {}", str, e);
								return Ok(Auth {
									developer: None,
									token: None,
								});
							}
							Ok(token) => token,
						}
					}
				},
			};

			let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
			let hash = sha256::digest(token.to_string());
			let developer = sqlx::query_as!(
				FetchedDeveloper,
				"SELECT d.id, d.username, d.display_name, d.verified, d.admin FROM developers d
                INNER JOIN auth_tokens a ON d.id = a.developer_id
                WHERE a.token = $1",
				hash
			)
			.fetch_optional(&mut *pool)
			.await;
			let developer = match developer {
				Err(e) => {
					log::error!("{}", e);
					return Err(ApiError::DbError);
				}
				Ok(d) => match d {
					None => {
						return Ok(Auth {
							developer: None,
							token: None,
						})
					}
					Some(data) => data,
				},
			};

			Ok(Auth {
				developer: Some(developer),
				token: Some(token),
			})
		})
	}
}
