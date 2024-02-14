use std::pin::Pin;

use actix_web::{web, FromRequest, HttpRequest};
use futures::Future;
use uuid::Uuid;

use crate::{
    types::{api::ApiError, models::developer::FetchedDeveloper},
    AppData,
};

pub struct Auth {
    pub developer: FetchedDeveloper,
}

impl FromRequest for Auth {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        let data = req.app_data::<web::Data<AppData>>().unwrap().clone();
        let headers = req.headers().clone();
        Box::pin(async move {
            let token = match headers.get("Authorization") {
                None => return Err(ApiError::Unauthorized),
                Some(t) => match t.to_str() {
                    Err(e) => {
                        log::error!("Failed to parse auth token: {}", e);
                        return Err(ApiError::Unauthorized);
                    }
                    Ok(str) => {
                        let split = str.split(" ").collect::<Vec<&str>>();
                        if split.len() != 2 || split[0] != "Bearer" {
                            return Err(ApiError::Unauthorized);
                        }
                        match Uuid::try_parse(split[1]) {
                            Err(e) => {
                                log::error!("Failed to parse auth token {}, error: {}", str, e);
                                return Err(ApiError::Unauthorized);
                            }
                            Ok(token) => token,
                        }
                    }
                },
            };

            let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;
            let developer = sqlx::query_as!(
                FetchedDeveloper,
                "SELECT d.id, d.username, d.display_name, d.verified, d.admin FROM developers d
                INNER JOIN auth_tokens at ON at.developer_id = d.id
                WHERE at.token = $1",
                token
            )
            .fetch_optional(&mut *pool)
            .await;
            let developer = match developer {
                Err(e) => {
                    log::error!("{}", e);
                    return Err(ApiError::DbError);
                }
                Ok(d) => match d {
                    None => return Err(ApiError::Unauthorized),
                    Some(data) => data,
                },
            };

            Ok(Auth { developer })
        })
    }
}
