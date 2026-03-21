use crate::config::AppData;
use crate::endpoints::ApiError;
use actix_web::{HttpResponse, Responder, get, web};
use serde::Deserialize;

use std::fs;
use std::path::Path;
use urlencoding;

const LABEL_COLOR: &str = "#0c0811";
const STAT_COLOR: &str = "#5f3d84";

#[derive(Deserialize)]
pub struct StatusBadgeQuery {
    pub stat: String,
}

#[utoipa::path(
    get,
    path = "/v1/mods/{id}/status_badge",
    tag = "mods",
    params(
        ("id" = String, Path, description = "Mod ID"),
        ("stat" = String, Query, description = "Stat to display: version, gd_version, geode_version, downloads")
    ),
    responses(
        (status = 302, description = "Redirect to Shields.io badge"),
        (status = 400, description = "Invalid stat or missing parameter"),
        (status = 404, description = "Mod not found")
    )
)]
#[get("/v1/mods/{id}/status_badge")]
pub async fn status_badge(
    _data: web::Data<AppData>,
    id: web::Path<String>,
    query: web::Query<StatusBadgeQuery>,
) -> Result<impl Responder, ApiError> {
    let (stat, label, svg_path) = match query.stat.as_str() {
        "version" => (
            "payload.versions[0].version",
            "Version",
            "static/mod_version.svg",
        ),
        "gd_version" => (
            "payload.versions[0].gd.win",
            "Geometry Dash",
            "static/mod_gd_version.svg",
        ),
        "geode_version" => (
            "payload.versions[0].geode",
            "Geode",
            "static/mod_geode_version.svg",
        ),
        "downloads" => (
            "payload.download_count",
            "Downloads",
            "static/mod_downloads.svg",
        ),
        _ => return Err(ApiError::BadRequest("Invalid stat parameter".into())),
    };
    let svg = fs::read_to_string(Path::new(svg_path))
        .map_err(|_| ApiError::BadRequest(format!("Could not read SVG file: {}", svg_path)))?;
    let api_url = format!(
        "{}/v1/mods/{}?abbreviate=true",
        "http://api.geode-sdk.org", id
    );
    let mod_link = format!("https://geode-sdk.org/mods/{}", id);
    let svg_data_url = format!("data:image/svg+xml;utf8,{}", urlencoding::encode(&svg));
    let shields_url = format!(
        "https://img.shields.io/badge/dynamic/json?url={}&query={}&label={}&labelColor={}&color={}&link={}&style=plastic&logo={}",
        urlencoding::encode(&api_url),
        urlencoding::encode(stat),
        label,
        urlencoding::encode(LABEL_COLOR),
        urlencoding::encode(STAT_COLOR),
        urlencoding::encode(&mod_link),
        urlencoding::encode(&svg_data_url)
    );
    Ok(HttpResponse::Found()
        .append_header(("Location", shields_url))
        .finish())
}
