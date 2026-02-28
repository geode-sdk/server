use utoipa::OpenApi;

use crate::{endpoints, types};

#[derive(OpenApi)]
#[openapi(
    paths(
        endpoints::mods::index,
        endpoints::mods::get,
        endpoints::mods::create,
        endpoints::mods::update_mod,
        endpoints::mods::get_logo,
        endpoints::mods::get_mod_updates,
        endpoints::mod_versions::get_version_index,
        endpoints::mod_versions::get_one,
        endpoints::mod_versions::download_version,
        endpoints::mod_versions::create_version,
        endpoints::mod_versions::update_version,
        endpoints::deprecations::index,
        endpoints::deprecations::store,
        endpoints::deprecations::update,
        endpoints::deprecations::delete,
        endpoints::deprecations::clear_all,
        endpoints::developers::developer_index,
        endpoints::developers::get_developer,
        endpoints::developers::add_developer_to_mod,
        endpoints::developers::remove_dev_from_mod,
        endpoints::developers::delete_token,
        endpoints::developers::delete_tokens,
        endpoints::developers::update_profile,
        endpoints::developers::get_own_mods,
        endpoints::developers::get_me,
        endpoints::developers::update_developer,
        endpoints::tags::index,
        endpoints::tags::detailed_index,
        endpoints::stats::get_stats,
        endpoints::loader::get_one,
        endpoints::loader::create_version,
        endpoints::loader::get_many,
        endpoints::health::health,
        endpoints::auth::refresh_token,
        endpoints::auth::github::start_github_login,
        endpoints::auth::github::start_github_web_login,
        endpoints::auth::github::github_web_callback,
        endpoints::auth::github::poll_github_login,
        endpoints::auth::github::github_token_login,
    ),
    components(
        schemas(
            types::api::ApiResponse<String>,
            types::api::PaginatedData<types::models::mod_entity::Mod>,
            types::api::PaginatedData<types::models::developer::Developer>,
            types::api::PaginatedData<types::models::loader_version::LoaderVersion>,
            types::models::mod_entity::Mod,
            types::models::mod_entity::ModUpdate,
            types::models::mod_version::ModVersion,
            types::models::developer::ModDeveloper,
            types::models::developer::Developer,
            types::models::deprecations::Deprecation,
            types::models::tag::Tag,
            types::models::stats::Stats,
            types::models::mod_version_status::ModVersionStatusEnum,
            types::models::mod_gd_version::GDVersionEnum,
            types::models::mod_gd_version::VerPlatform,
            types::models::mod_gd_version::DetailedGDVersion,
            types::models::dependency::ResponseDependency,
            types::models::dependency::ModVersionCompare,
            types::models::dependency::DependencyImportance,
            types::models::incompatibility::ResponseIncompatibility,
            types::models::incompatibility::Replacement,
            types::models::incompatibility::IncompatibilityImportance,
            types::models::mod_link::ModLinks,
            types::models::loader_version::LoaderVersion,
            types::models::gd_version_alias::GDVersionAlias,
            endpoints::mods::IndexSortType,
            endpoints::developers::SimpleDevMod,
            endpoints::developers::SimpleDevModVersion,
        )
    ),
    tags(
        (name = "mods", description = "Mod management endpoints"),
        (name = "mod_versions", description = "Mod version management endpoints"),
        (name = "deprecations", description = "Mod deprecation management endpoints"),
        (name = "developers", description = "Developer management endpoints"),
        (name = "tags", description = "Tag management endpoints"),
        (name = "stats", description = "Statistics endpoints"),
        (name = "loader", description = "Geode loader version endpoints"),
        (name = "auth", description = "Authentication endpoints"),
        (name = "health", description = "Health check endpoint"),
    ),
    info(
        title = "Geode Index API",
        version = "0.51.2",
        description = "API for the Geode mod index",
        contact(
            name = "Geode Team",
            url = "https://geode-sdk.org"
        )
    ),
    servers(
        (url = "https://api.geode-sdk.org", description = "Geode index")
    )
)]
pub struct ApiDoc;
