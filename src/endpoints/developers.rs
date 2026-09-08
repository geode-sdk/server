use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use argon2::password_hash::{SaltString, rand_core::OsRng};
use argon2::{Argon2, PasswordHasher};
use maud::html;
use serde::{Deserialize, Serialize};
use sqlx::Connection;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

use std::str::FromStr;

use super::ApiError;
use crate::config::AppData;
use crate::database::repository::{
    auth_tokens, developers, email_setup_requests, mods, refresh_tokens,
};
use crate::email::EmailAddress;
use crate::email::blocklist::ApprovedEmailAddress;
use crate::email::mailer::{EmailBody, OutgoingEmail};
use crate::types::api::{ApiResponse, PaginatedData};
use crate::types::models::developer::SelfDeveloper;
use crate::{
    extractors::auth::Auth,
    types::models::{
        developer::{Developer, ModDeveloper},
        mod_entity::Mod,
        mod_version_status::ModVersionStatusEnum,
    },
};

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
pub struct SimpleDevMod {
    pub id: String,
    pub featured: bool,
    pub download_count: i32,
    pub versions: Vec<SimpleDevModVersion>,
    pub developers: Vec<ModDeveloper>,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
pub struct SimpleDevModVersion {
    pub name: String,
    pub version: String,
    pub download_count: i32,
    pub validated: bool,
    pub info: Option<String>,
    pub status: ModVersionStatusEnum,
}

#[derive(Deserialize, IntoParams)]
struct AddDevPath {
    id: String,
}

#[derive(Deserialize, IntoParams)]
struct RemoveDevPath {
    id: String,
    username: String,
}

#[derive(Deserialize, ToSchema)]
struct AddDevPayload {
    username: String,
}

#[derive(Deserialize, ToSchema)]
struct DeveloperUpdatePayload {
    admin: Option<bool>,
    verified: Option<bool>,
}

#[derive(Deserialize, IntoParams)]
struct UpdateDeveloperPath {
    id: i32,
}

#[derive(Deserialize, IntoParams)]
struct DeveloperIndexQuery {
    query: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

/// List all developers with optional search and pagination
#[utoipa::path(
    get,
    path = "/v1/developers",
    tag = "developers",
    params(DeveloperIndexQuery),
    responses(
        (status = 200, description = "List of developers", body = inline(ApiResponse<PaginatedData<Developer>>))
    )
)]
#[get("v1/developers")]
#[tracing::instrument(skip_all)]
pub async fn developer_index(
    data: web::Data<AppData>,
    query: web::Query<DeveloperIndexQuery>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db().acquire().await?;

    let page: i64 = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

    let query = query.query.clone();

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: developers::index(query.as_deref(), page, per_page, &mut pool).await?,
    }))
}

/// Add a developer to a mod
#[utoipa::path(
    post,
    path = "/v1/mods/{id}/developers",
    tag = "developers",
    params(AddDevPath),
    request_body = AddDevPayload,
    responses(
        (status = 204, description = "Developer added successfully"),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Mod or developer not found")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[post("v1/mods/{id}/developers")]
#[tracing::instrument(skip_all, fields(mod_id = %path.id, username = %json.username))]
pub async fn add_developer_to_mod(
    data: web::Data<AppData>,
    path: web::Path<AddDevPath>,
    json: web::Json<AddDevPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }
    if !developers::owns_mod(dev.id, &path.id, &mut pool).await? {
        return Err(ApiError::Authorization);
    }

    let target = developers::get_one_by_username(&json.username, &mut pool)
        .await?
        .ok_or(ApiError::BadRequest(format!(
            "No developer found with username {}",
            json.username
        )))?;

    mods::assign_developer(&path.id, target.id, false, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}

/// Remove a developer from a mod
#[utoipa::path(
    delete,
    path = "/v1/mods/{id}/developers/{username}",
    tag = "developers",
    params(RemoveDevPath),
    responses(
        (status = 204, description = "Developer removed successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Mod or developer not found"),
        (status = 409, description = "Cannot remove self")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[delete("v1/mods/{id}/developers/{username}")]
#[tracing::instrument(skip_all, fields(mod_id = %path.id, username = %path.username))]
pub async fn remove_dev_from_mod(
    data: web::Data<AppData>,
    path: web::Path<RemoveDevPath>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod id {} not found", path.id)));
    }

    if !developers::owns_mod(dev.id, &path.id, &mut pool).await? {
        return Err(ApiError::Authorization);
    }

    let target = developers::get_one_by_username(&path.username, &mut pool)
        .await?
        .ok_or(ApiError::NotFound(format!(
            "No developer found with username {}",
            path.username
        )))?;

    if target.id == dev.id {
        return Ok(HttpResponse::Conflict().json(ApiResponse {
            error: "Cannot remove self from mod developer list".into(),
            payload: "",
        }));
    }

    if !developers::has_access_to_mod(target.id, &path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!(
            "{} is not a developer for this mod",
            target.username
        )));
    }

    mods::unassign_developer(&path.id, target.id, &mut pool).await?;

    Ok(HttpResponse::NoContent().finish())
}

/// Delete the current API token
#[utoipa::path(
    delete,
    path = "/v1/me/token",
    tag = "developers",
    responses(
        (status = 204, description = "Token deleted successfully"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[delete("v1/me/token")]
#[tracing::instrument(skip_all)]
pub async fn delete_token(
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let token = auth.token()?;
    let mut pool = data.db().acquire().await?;

    auth_tokens::remove_token(token, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}

/// Delete all API tokens for the current developer
#[utoipa::path(
    delete,
    path = "/v1/me/tokens",
    tag = "developers",
    responses(
        (status = 204, description = "All tokens deleted successfully"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[delete("v1/me/tokens")]
#[tracing::instrument(skip_all)]
pub async fn delete_tokens(
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;

    auth_tokens::remove_developer_tokens(dev.id, &mut pool).await?;
    refresh_tokens::remove_developer_tokens(dev.id, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}

#[derive(Deserialize, ToSchema)]
struct UploadProfilePayload {
    display_name: String,
}

/// Update the current developer's profile
#[utoipa::path(
    put,
    path = "/v1/me",
    tag = "developers",
    request_body = UploadProfilePayload,
    responses(
        (status = 200, description = "Profile updated", body = inline(ApiResponse<Developer>)),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[put("v1/me")]
#[tracing::instrument(skip_all)]
pub async fn update_profile(
    data: web::Data<AppData>,
    json: web::Json<UploadProfilePayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;

    if !json
        .display_name
        .chars()
        .all(|x| char::is_ascii_alphanumeric(&x))
    {
        return Err(ApiError::BadRequest(
            "Display name must contain only ASCII alphanumeric characters".into(),
        ));
    }

    if json.display_name.len() < 2 || json.display_name.len() > 64 {
        return Err(ApiError::BadRequest(
            "Display name must be between 2 and 64 characters".into(),
        ));
    }

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: developers::update_profile(dev.id, &json.display_name, &mut pool).await?,
    }))
}

#[derive(Deserialize, IntoParams)]
struct GetOwnModsQuery {
    #[serde(default = "default_own_mods_status")]
    status: ModVersionStatusEnum,
    #[serde(default)]
    only_owner: bool,
}

pub fn default_own_mods_status() -> ModVersionStatusEnum {
    ModVersionStatusEnum::Accepted
}

/// Get all mods owned by the current developer
#[utoipa::path(
    get,
    path = "/v1/me/mods",
    tag = "developers",
    params(GetOwnModsQuery),
    responses(
        (status = 200, description = "List of developer's mods", body = inline(ApiResponse<Vec<SimpleDevMod>>)),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[get("v1/me/mods")]
#[tracing::instrument(skip_all)]
pub async fn get_own_mods(
    data: web::Data<AppData>,
    query: web::Query<GetOwnModsQuery>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    let mut pool = data.db().acquire().await?;
    let mods: Vec<SimpleDevMod> =
        Mod::get_all_for_dev(dev.id, query.status, query.only_owner, &mut pool).await?;
    Ok(HttpResponse::Ok().json(ApiResponse {
        error: "".to_string(),
        payload: mods,
    }))
}

/// Get the current developer's profile
#[utoipa::path(
    get,
    path = "/v1/me",
    tag = "developers",
    responses(
        (status = 200, description = "Current developer profile", body = inline(ApiResponse<SelfDeveloper>)),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[get("v1/me")]
#[tracing::instrument(skip_all)]
pub async fn get_me(auth: Auth, data: web::Data<AppData>) -> Result<impl Responder, ApiError> {
    let mut pool = data.db().acquire().await?;
    let dev = auth.developer()?;

    let has_accepted_mod = developers::has_accepted_mod(dev.id, &mut pool).await?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        error: "".to_string(),
        payload: dev.to_self_developer(has_accepted_mod),
    }))
}

#[derive(Deserialize, ToSchema, Validate)]
struct StartEmailConfigurationPayload {
    #[validate(email)]
    email: String,
    #[validate(
        length(min = 8),
        contains(pattern = "[0-9]", message = "password must contain at least 1 digit")
    )]
    password: String,
    /// Force remove the existing request if not expired
    force: Option<bool>,
}

/// Start email configuration
#[utoipa::path(
    post,
    path = "/v1/me/email/setup",
    request_body = StartEmailConfigurationPayload,
    tag = "developers",
    responses(
        (status = 201, description = "Email setup request started", body = inline(ApiResponse<String>)),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "E-mail already configured")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[post("v1/me/email/setup")]
#[tracing::instrument(skip_all)]
pub async fn setup_email(
    auth: Auth,
    data: web::Data<AppData>,
    json: web::Json<StartEmailConfigurationPayload>,
) -> Result<impl Responder, ApiError> {
    let mailer = data.mailer().ok_or(ApiError::NotImplemented)?;
    let front_url = data.front_url();

    // We need to redirect the user to a frontend URL.
    // If by chance we don't have a base URL, let's just lock the implementation
    if front_url.is_empty() {
        return Err(ApiError::NotImplemented);
    }

    let developer = auth.developer()?;

    // We're trying to keep the DB connection occupied as little as possible.
    // So do this chunk, send the email with no DB connection hogged,
    // then get another connection to finish the insert
    {
        let mut conn = data.db().acquire().await?;

        // Prevent someone stealing an address
        let existing = developers::find_by_email(&json.email, &mut conn).await?;

        if existing.is_some_and(|e| e.id != developer.id) {
            return Err(ApiError::Conflict(
                "Developer already exists with this email address".into(),
            ));
        }

        let force = json.force.unwrap_or(false);
        let existing = email_setup_requests::find_for_developer(developer.id, &mut conn).await?;

        if existing.is_some() && !force {
            return Err(ApiError::Conflict(
                "user already has an active email setup request".into(),
            ));
        }
    }

    let email = ApprovedEmailAddress::parse(
        EmailAddress::from_str(&json.email).expect("email validated by json struct"),
        data.email_blocklist(),
    )?;

    let salt = SaltString::generate(&mut OsRng);
    let password = json.password.clone();

    let hash = tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .inspect_err(|e| tracing::error!("{:?}", e))
            .map_err(|_| ApiError::InternalError("failed to hash password".into()))
            .map(|hash| hash.serialize())
            .map_err(|_| ApiError::InternalError("failed to hash password".into()))
    })
    .await
    .map_err(|e| ApiError::InternalError(format!("hashing task panicked: {e}")))??;

    let uuid = Uuid::new_v4();

    let endpoint = format!("{}/email/verify?token={}", data.front_url(), uuid);

    let html = html! {
        p { "Hi " (developer.display_name) "," }
        p {
            "Someone (hopefully you) requested to setup email sign-in for your Geode SDK developer account using this address."
        }
        p {
            "If you didn't request this, you can safely ignore this email. No changes will be made to your account."
        }
        p {
            a href=(endpoint) { "Confirm this email address" }
        }
        p {
            "Or paste this link into your browser:" br;
            (endpoint)
        }
        p {
            "This link expires in 30 minutes."
        }
    }
    .into_string();

    let outgoing = OutgoingEmail {
        to: email.clone(),
        subject: "Email setup | Geode SDK".into(),
        body: EmailBody::html_with_autogenerated_text(html)?,
    };

    mailer
        .send(&outgoing)
        .await
        .inspect_err(|e| tracing::error!("{:?}", e))?;

    let mut pool = data.db().acquire().await?;
    let mut tx = pool.begin().await?;
    email_setup_requests::delete_for_developer(developer.id, &mut tx).await?;
    email_setup_requests::create(uuid, developer.id, &email, hash, &mut tx).await?;
    tx.commit().await?;

    Ok(HttpResponse::Created().json(ApiResponse {
        payload: "".to_string(),
        error: "".into(),
    }))
}

#[derive(Deserialize, ToSchema, Validate)]
struct VerifyEmailConfigurationPayload {
    request_id: String,
}

/// Finalize email configuration
#[utoipa::path(
    post,
    path = "/v1/me/email/setup/verify",
    request_body = VerifyEmailConfigurationPayload,
    tag = "developers",
    responses(
        (status = 200, description = "Email settings applied", body = inline(ApiResponse<String>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Setup request not found"),
        (status = 409, description = "E-mail already configured")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[post("v1/me/email/setup/verify")]
#[tracing::instrument(skip_all)]
pub async fn verify_email_setup(
    data: web::Data<AppData>,
    json: web::Json<VerifyEmailConfigurationPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let developer = auth.developer()?;

    let request_id = Uuid::try_parse(&json.request_id).map_err(|_| {
        ApiError::NotFound(
            "The request associated with this ID either does not exist or has expired".into(),
        )
    })?;

    let mut conn = data.db().acquire().await?;

    let mut tx = conn.begin().await?;

    let request = email_setup_requests::find_for_token(request_id, &mut tx)
        .await?
        .ok_or(ApiError::NotFound(
            "The request associated with this ID either does not exist or has expired".into(),
        ))?;

    if request.developer_id != developer.id {
        // Let's be a little sneaky here... >:)
        return Err(ApiError::NotFound(
            "The request associated with this ID either does not exist or has expired".into(),
        ));
    }

    let existing = developers::find_by_email(&request.email, &mut tx).await?;

    if existing.is_some_and(|e| e.id != developer.id) {
        return Err(ApiError::Conflict(
            "Developer already exists with this email address".into(),
        ));
    }

    // Who knows, maybe the blocklist gets updated while the request is running
    let email = ApprovedEmailAddress::parse(
        EmailAddress::from_str(&request.email)
            .inspect_err(|e| tracing::error!("failed to parse email from EmailSetupRequest: {e}"))
            .map_err(|e| {
                ApiError::InternalError(format!(
                    "Failed to read email from email setup request: {e}"
                ))
            })?,
        data.email_blocklist(),
    )?;

    // This should basically never fail
    let password = request.password()
        .inspect_err(|e| tracing::error!("Invalid hashed password in EmailSetupRequest: {e}"))
        .map_err(|_| ApiError::InternalError("Failed to read password from email setup request. Please try setting up your email address again".into()))?;

    developers::finalize_email_setup(developer.id, &email, &password, &mut tx).await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        payload: "".to_string(),
        error: "".into(),
    }))
}

#[derive(Deserialize, IntoParams)]
struct GetDeveloperPath {
    id: i32,
}

/// Get a specific developer by ID
#[utoipa::path(
    get,
    path = "/v1/developers/{id}",
    tag = "developers",
    params(GetDeveloperPath),
    responses(
        (status = 200, description = "Developer details", body = inline(ApiResponse<Developer>)),
        (status = 404, description = "Developer not found")
    )
)]
#[get("v1/developers/{id}")]
#[tracing::instrument(skip_all, fields(developer_id = %path.id))]
pub async fn get_developer(
    data: web::Data<AppData>,
    path: web::Path<GetDeveloperPath>,
) -> Result<impl Responder, ApiError> {
    let mut pool = data.db().acquire().await?;
    let result = developers::get_one(path.id, &mut pool)
        .await?
        .ok_or(ApiError::NotFound("Developer not found".into()))?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: result,
    }))
}

/// Update a developer's admin/verified status (admin only)
#[utoipa::path(
    put,
    path = "/v1/developers/{id}",
    tag = "developers",
    params(UpdateDeveloperPath),
    request_body = DeveloperUpdatePayload,
    responses(
        (status = 200, description = "Developer updated", body = inline(ApiResponse<Developer>)),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin only"),
        (status = 404, description = "Developer not found")
    ),
    security(
        ("bearer_token" = [])
    )
)]
#[put("v1/developers/{id}")]
#[tracing::instrument(skip_all, fields(developer_id = %path.id))]
pub async fn update_developer(
    auth: Auth,
    data: web::Data<AppData>,
    path: web::Path<UpdateDeveloperPath>,
    payload: web::Json<DeveloperUpdatePayload>,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    auth.check_admin()?;

    if payload.admin.is_none() && payload.verified.is_none() {
        return Err(ApiError::BadRequest(
            "Specify at least one param to modify".into(),
        ));
    }

    let mut pool = data.db().acquire().await?;

    if payload.admin.is_some() && dev.id == path.id {
        return Err(ApiError::BadRequest(
            "Can't override your own admin status".into(),
        ));
    }

    let updating = {
        if dev.id == path.id {
            dev
        } else {
            developers::get_one(path.id, &mut pool)
                .await?
                .ok_or(ApiError::NotFound("Developer not found".into()))?
        }
    };

    let result = developers::update_status(
        path.id,
        payload.verified.unwrap_or(updating.verified),
        payload.admin.unwrap_or(updating.admin),
        &mut pool,
    )
    .await?;

    Ok(web::Json(ApiResponse {
        error: "".to_string(),
        payload: result,
    }))
}
