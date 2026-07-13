use tracing_subscriber::EnvFilter;

/// Initialize the global `tracing` subscriber.
///
/// The filter is controlled by the `RUST_LOG` env var, falling back to
/// `"info,sqlx=warn,tracing_actix_web::middleware=error"` when unset. This
/// default is identical for debug and release builds.
///
/// The output format is controlled by the `LOG_FORMAT` env var: when it is
/// exactly `"json"`, logs are emitted as JSON; otherwise the default
/// compact/full text format is used.
pub fn init() {
    const DEFAULT_VALUE: &str = "info,sqlx=warn,tracing_actix_web::middleware=error";

    let filter = EnvFilter::from(dotenvy::var("RUST_LOG").unwrap_or(DEFAULT_VALUE.into()));

    let is_json = dotenvy::var("LOG_FORMAT")
        .map(|v| v == "json")
        .unwrap_or(false);

    if is_json {
        tracing_subscriber::fmt()
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .with_env_filter(filter)
            .init();
    }
}
