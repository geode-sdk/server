use std::future::Ready;

use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures::future;
use futures::future::LocalBoxFuture;

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
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SignedUrlMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ok(SignedUrlMiddleware { service })
    }
}

pub struct SignedUrlMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SignedUrlMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let fut = self.service.call(req);

        Box::pin(async move { Ok(fut.await?) })
    }
}
