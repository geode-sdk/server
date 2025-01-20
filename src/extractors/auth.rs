use std::pin::Pin;

use crate::config::AppData;
use crate::types::{api::ApiError, models::developer::Developer};
use actix_web::http::header::HeaderMap;
use actix_web::{web, FromRequest, HttpRequest};
use futures::Future;
use uuid::Uuid;

pub struct Auth {
    developer: Option<Developer>,
    token: Option<Uuid>,
}

impl Auth {
    /**
     * Returns Ok(developer) if token was valid in request or returns ApiError::Unauthorized otherwise
     */
    pub fn developer(&self) -> Result<Developer, ApiError> {
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

    pub fn admin(&self) -> Result<(), ApiError> {
        if self.developer.is_none() {
            return Err(ApiError::Unauthorized);
        }

        match self.developer.as_ref().is_some_and(|dev| dev.admin) {
            false => Err(ApiError::Forbidden),
            true => Ok(())
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
            let token = match parse_token(&headers) {
                Some(t) => t,
                None => {
                    return Ok(Auth {
                        developer: None,
                        token: None,
                    });
                }
            };

            let mut pool = data
                .db()
                .acquire()
                .await
                .or(Err(ApiError::DbAcquireError))?;
            let hash = sha256::digest(token.to_string());
            let developer = match sqlx::query_as!(
                Developer,
                "SELECT
                    d.id,
                    d.username,
                    d.display_name,
                    d.verified,
                    d.admin
                FROM developers d
                INNER JOIN auth_tokens a ON d.id = a.developer_id
                WHERE a.token = $1",
                hash
            )
            .fetch_optional(&mut *pool)
            .await
            .map_err(|e| {
                log::error!("Failed to lookup developer for auth: {}", e);
                ApiError::DbError
            })? {
                None => {
                    return Ok(Auth {
                        developer: None,
                        token: None,
                    })
                }
                Some(d) => d,
            };

            Ok(Auth {
                developer: Some(developer),
                token: Some(token),
            })
        })
    }
}

fn parse_token(map: &HeaderMap) -> Option<Uuid> {
    map.get("Authorization")
        .map(|header| header.to_str().ok())
        .flatten()
        .map(|str| -> Option<&str> {
            let split = str.split(' ').collect::<Vec<&str>>();
            if split.len() != 2 || split[0] != "Bearer" {
                None
            } else {
                Some(split[1])
            }
        })
        .flatten()
        .map(|str| Uuid::try_parse(str).ok())
        .flatten()
}
