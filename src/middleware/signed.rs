use std::future::{Ready, ready};

use actix_web::{
    Error, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures::future::LocalBoxFuture;
use http::StatusCode;

use crate::{types::api::ApiResponse, urisign};

pub struct SignedUrl {
    signing_key: String,
}

impl SignedUrl {
    pub fn new(signing_key: String) -> Self {
        Self { signing_key }
    }
}

impl<S, B> Transform<S, ServiceRequest> for SignedUrl
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = SignedUrlMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SignedUrlMiddleware {
            service,
            signing_key: self.signing_key.clone(),
        }))
    }
}

pub struct SignedUrlMiddleware<S> {
    service: S,
    signing_key: String,
}

impl<S, B> Service<ServiceRequest> for SignedUrlMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if let Err(e) = urisign::check_signed_uri(req.uri(), &self.signing_key) {
            let response = match e {
                urisign::CheckUriError::Expired => {
                    HttpResponse::build(StatusCode::GONE).json(ApiResponse {
                        error: format!("{}", e),
                        payload: "",
                    })
                }
                urisign::CheckUriError::Invalid => {
                    HttpResponse::build(StatusCode::NOT_FOUND).json(ApiResponse {
                        error: "Not found".into(),
                        payload: "",
                    })
                }
            };

            let service_response = req.into_response(response).map_into_right_body();
            return Box::pin(ready(Ok(service_response)));
        }

        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;

            Ok(res.map_into_left_body())
        })
    }
}
