use actix_cors::Cors;
use actix_web::{
    App, HttpMessage, HttpServer,
    body::MessageBody,
    dev::{Service, ServiceFactory, ServiceRequest, ServiceResponse},
    web::{self, QueryConfig},
};
use std::sync::Arc;
use tracing_actix_web::RequestId;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

use crate::{
    config::AppData,
    http::endpoints::{
        auth, deprecations, developers, email_change_requests, health, loader, mod_status_badge, mod_version_submissions,
        mod_versions, mods, stats, tags,
    },
    http::{error::query_error_handler, openapi::ApiDoc},
};

pub mod endpoints;
pub mod error;
pub mod extractors;
pub mod response;

mod openapi;

tokio::task_local! {
    pub static REQUEST_ID: Uuid;
}

fn router_config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .configure(mods::router_config)
            .configure(mod_status_badge::router_config)
            .configure(mod_versions::router_config)
            .configure(mod_version_submissions::router_config)
            .configure(deprecations::router_config)
            .configure(auth::router_config)
            .configure(developers::router_config)
            .configure(tags::router_config)
            .configure(stats::router_config)
            .configure(email_change_requests::router_config)
            .configure(loader::router_config),
    )
    .configure(health::router_config);
}

pub fn cors_settings() -> Cors {
    Cors::default()
        .allow_any_origin()
        .allowed_methods(vec!["GET", "HEAD"])
        .allowed_headers(vec![actix_web::http::header::ACCEPT])
        .max_age(3600)
}

pub fn create_app(
    data: web::Data<AppData>,
    openapi: utoipa::openapi::OpenApi,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<impl MessageBody>,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let app = App::new()
        .app_data(data.clone())
        .app_data(QueryConfig::default().error_handler(query_error_handler))
        .service(SwaggerUi::new("/swagger/{_:.*}").url("/swagger/openapi.json", openapi.clone()))
        .wrap(cors_settings())
        .wrap_fn(|req, srv| {
            let request_id = req.extensions().get::<RequestId>().map(|id| **id);
            let fut = srv.call(req);
            async move {
                match request_id {
                    Some(id) => REQUEST_ID.scope(id, fut).await,
                    None => fut.await,
                }
            }
        })
        .wrap(tracing_actix_web::TracingLogger::default());

    #[cfg(feature = "serve-static")]
    let app = app
        .service(actix_files::Files::new("/static", "static"))
        .service(actix_files::Files::new("/storage", "storage/public"));

    app.configure(router_config)
}

pub async fn serve_http(data: Arc<AppData>) -> anyhow::Result<()> {
    let data = web::Data::from(data);
    let port = data.port();
    let debug = data.debug();

    tracing::info!("Starting server on 0.0.0.0:{}", port);

    let openapi = ApiDoc::openapi();
    let server = HttpServer::new(move || create_app(data.clone(), openapi.clone()))
        .bind(("0.0.0.0", port))?;

    if debug {
        tracing::info!("Running in debug mode, using 1 thread.");
        server.workers(1).run().await?;
    } else {
        server.run().await?;
    }

    Ok(())
}
