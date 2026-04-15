use crate::config::AppData;
use crate::endpoints::ApiError;
use actix_web::{HttpResponse, Responder, get, web};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::storage::StorageDisk;
use urlencoding;

const LABEL_COLOR: &str = "#0c0811";
const STAT_COLOR: &str = "#5f3d84";

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatusBadgeStat {
    Version,
    GdVersion,
    GeodeVersion,
    Downloads,
}

#[derive(Deserialize, IntoParams)]
pub struct StatusBadgeQuery {
    pub stat: StatusBadgeStat,
}

#[utoipa::path(
    get,
    path = "/v1/mods/{id}/status_badge",
    tag = "mods",
    params(
        ("id" = String, Path, description = "Mod ID"),
        StatusBadgeQuery
    ),
    responses(
        (status = 302, description = "Redirect to Shields.io badge"),
        (status = 400, description = "Invalid stat or missing parameter"),
        (status = 404, description = "Mod not found")
    )
)]
#[get("/v1/mods/{id}/status_badge")]
pub async fn status_badge(
    data: web::Data<AppData>,
    id: web::Path<String>,
    query: web::Query<StatusBadgeQuery>,
) -> Result<impl Responder, ApiError> {
    let (stat, label, svg_path) = match query.stat {
        StatusBadgeStat::Version => (
            "payload.versions[0].version",
            "Version",
            "shields/mod_version.svg",
        ),
        StatusBadgeStat::GdVersion => (
            "payload.versions[0].gd.win",
            "Geometry Dash",
            "shields/mod_gd_version.svg",
        ),
        StatusBadgeStat::GeodeVersion => (
            "payload.versions[0].geode",
            "Geode",
            "shields/mod_geode_version.svg",
        ),
        StatusBadgeStat::Downloads => (
            "payload.download_count",
            "Downloads",
            "shields/mod_downloads.svg",
        ),
    };
    let svg_url = data.static_storage().asset_url(svg_path);
    let api_url = format!("{}/v1/mods/{}?abbreviate=true", data.app_url(), id);
    let mod_link = format!("{}/mods/{}", data.front_url(), id);
    let shields_url = format!(
        "https://img.shields.io/badge/dynamic/json?url={}&query={}&label={}&labelColor={}&color={}&link={}&style=plastic&logo={}",
        urlencoding::encode(&api_url),
        urlencoding::encode(stat),
        label,
        urlencoding::encode(LABEL_COLOR),
        urlencoding::encode(STAT_COLOR),
        urlencoding::encode(&mod_link),
        urlencoding::encode(&svg_url)
    );
    Ok(HttpResponse::Found()
        .append_header(("Location", shields_url))
        .finish())
}
