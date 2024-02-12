use actix_web::FromRequest;
use futures::{future::Ready, Future};

use crate::types::{api::ApiError, models::developer::Developer};

pub struct AuthedDev;

impl FromRequest for AuthedDev {
    type Error = ApiError;
    type Future = Ready<Result<Option<Developer>, ApiError>>;

    fn from_request(req: &actix_web::HttpRequest, payload: &mut actix_web::dev::Payload) -> Self::Future {

    }
}