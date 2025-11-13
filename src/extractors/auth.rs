use std::pin::Pin;

use crate::auth::AuthenticationError;
use crate::config::AppData;
use crate::database::repository::developers;
use crate::endpoints::ApiError;
use crate::types::models::developer::Developer;
use actix_web::http::header::HeaderMap;
use actix_web::{web, FromRequest, HttpRequest};
use futures::Future;
use uuid::Uuid;

pub struct Auth {
    developer: Option<Developer>,
    token: Option<Uuid>,
}

impl Auth {
    pub fn developer(&self) -> Result<Developer, AuthenticationError> {
        if self.token.is_none() {
            return Err(AuthenticationError::NoToken);
        }
        match &self.developer {
            None => Err(AuthenticationError::InvalidToken),
            Some(d) => Ok(d.clone()),
        }
    }

    fn check_auth(&self) -> Result<(), AuthenticationError> {
        if self.token.is_none() {
            return Err(AuthenticationError::NoToken);
        }
        if self.developer.is_none() {
            return Err(AuthenticationError::InvalidToken);
        }

        Ok(())
    }

    pub fn token(&self) -> Result<Uuid, AuthenticationError> {
        match self.token {
            None => Err(AuthenticationError::NoToken),
            Some(t) => Ok(t),
        }
    }

    pub fn check_admin(&self) -> Result<(), ApiError> {
        self.check_auth()?;

        match self.developer.as_ref().is_some_and(|dev| dev.admin) {
            false => Err(ApiError::Authorization),
            true => Ok(()),
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

            let mut pool = data.db().acquire().await?;
            match developers::find_by_token(&token, &mut pool).await? {
                None => Ok(Auth {
                    developer: None,
                    token: Some(token),
                }),
                Some(dev) => Ok(Auth {
                    developer: Some(dev),
                    token: Some(token),
                }),
            }
        })
    }
}

fn parse_token(map: &HeaderMap) -> Option<Uuid> {
    map.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|str| -> Option<&str> {
            let split = str.split(' ').collect::<Vec<&str>>();
            if split.len() != 2 || split[0] != "Bearer" {
                None
            } else {
                Some(split[1])
            }
        })
        .and_then(|str| Uuid::try_parse(str).ok())
}
