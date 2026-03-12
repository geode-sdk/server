use std::collections::HashMap;
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;
use sqlx::PgConnection;
use utoipa::IntoParams;

use super::ApiError;
use crate::config::AppData;
use crate::database::repository::{developers, mod_version_submissions, mod_versions, mods};
use crate::extractors::auth::Auth;
use crate::types::api::{ApiResponse, PaginatedData};
use crate::types::models::mod_version_submission::{
    CreateCommentPayload, ModVersionSubmission, ModVersionSubmissionComment, UpdateCommentPayload,
    UpdateSubmissionPayload,
};

#[derive(Deserialize, IntoParams)]
struct SubmissionPath {
    id: String,
    version: String,
}

#[derive(Deserialize, IntoParams)]
struct CommentPath {
    id: String,
    version: String,
    comment_id: i64,
}

#[derive(Deserialize, IntoParams)]
struct CommentsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

/// Resolve a mod-version's numeric id from its string version tag, and
/// return both it and the verified-mod id.
async fn resolve_version_id(
    mod_id: &str,
    version: &str,
    pool: &mut PgConnection,
) -> Result<i32, ApiError> {
    let ver = mod_versions::get_by_version_str(mod_id, version, pool)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Version {} not found", version)))?;
    Ok(ver.id)
}

/// Get the submission for a mod version
#[utoipa::path(
    get,
    path = "/v1/mods/{id}/versions/{version}/submission",
    tag = "mod_version_submissions",
    params(SubmissionPath),
    responses(
        (status = 200, description = "Submission details", body = inline(ApiResponse<ModVersionSubmission>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Mod, version, or submission not found"),
    ),
    security(("bearer_token" = []))
)]
#[get("v1/mods/{id}/versions/{version}/submission")]
pub async fn get_submission(
    path: web::Path<SubmissionPath>,
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    auth.developer()?;

    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    let version_id = resolve_version_id(&path.id, &path.version, &mut pool).await?;

    let row = mod_version_submissions::get_for_mod_version(version_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Submission not found".into()))?;

    let locked_by = match row.locked_by {
        Some(dev_id) => Some(
            developers::get_one(dev_id, &mut pool)
                .await?
                .ok_or_else(|| ApiError::InternalError("Locked-by developer not found".into()))?,
        ),
        None => None,
    };

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: row.into_submission(locked_by),
    }))
}

/// Update (lock / unlock) a submission (admin only)
#[utoipa::path(
    put,
    path = "/v1/mods/{id}/versions/{version}/submission",
    tag = "mod_version_submissions",
    params(SubmissionPath),
    request_body = UpdateSubmissionPayload,
    responses(
        (status = 200, description = "Submission updated", body = inline(ApiResponse<ModVersionSubmission>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Mod, version, or submission not found"),
    ),
    security(("bearer_token" = []))
)]
#[put("v1/mods/{id}/versions/{version}/submission")]
pub async fn update_submission(
    path: web::Path<SubmissionPath>,
    data: web::Data<AppData>,
    payload: web::Json<UpdateSubmissionPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;
    auth.check_admin()?;
    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    let version_id = resolve_version_id(&path.id, &path.version, &mut pool).await?;

    mod_version_submissions::get_for_mod_version(version_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Submission not found".into()))?;

    let locked_by_id = if payload.locked {
        Some(dev.id)
    } else {
        None
    };

    let row =
        mod_version_submissions::set_locked(version_id, payload.locked, locked_by_id, &mut pool)
            .await?;

    let locked_by = if payload.locked {
        Some(dev.clone())
    } else {
        None
    };

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: row.into_submission(locked_by),
    }))
}

/// List comments for a mod version submission
#[utoipa::path(
    get,
    path = "/v1/mods/{id}/versions/{version}/submission/comments",
    tag = "mod_version_submissions",
    params(SubmissionPath, CommentsQuery),
    responses(
        (status = 200, description = "List of comments", body = inline(ApiResponse<PaginatedData<ModVersionSubmissionComment>>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Mod, version, or submission not found"),
    ),
    security(("bearer_token" = []))
)]
#[get("v1/mods/{id}/versions/{version}/submission/comments")]
pub async fn get_comments(
    path: web::Path<SubmissionPath>,
    data: web::Data<AppData>,
    query: web::Query<CommentsQuery>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    auth.developer()?;

    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    let version_id = resolve_version_id(&path.id, &path.version, &mut pool).await?;

    mod_version_submissions::get_for_mod_version(version_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Submission not found".into()))?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let count =
        mod_version_submissions::count_comments_for_submission(version_id, &mut pool).await?;
    let rows = mod_version_submissions::get_paginated_comments_for_submission(
        version_id, page, per_page, &mut pool,
    )
    .await?;

    let author_ids: Vec<i32> = {
        let mut ids: Vec<i32> = rows.iter().map(|r| r.author_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };

    let mut authors_map = developers::get_many_by_id(&author_ids, &mut pool)
        .await?
        .into_iter()
        .map(|dev| (dev.id, dev))
        .collect::<HashMap<_, _>>();

    let comments = rows
        .into_iter()
        .map(|row| {
            let author = authors_map
                .get(&row.author_id)
                .cloned()
                .ok_or_else(|| ApiError::InternalError("Author not found".into()))?;
            Ok(row.into_comment(author))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: PaginatedData {
            data: comments,
            count,
        },
    }))
}

/// Add a comment to a mod version submission
#[utoipa::path(
    post,
    path = "/v1/mods/{id}/versions/{version}/submission/comments",
    tag = "mod_version_submissions",
    params(SubmissionPath),
    request_body = CreateCommentPayload,
    responses(
        (status = 201, description = "Comment created", body = inline(ApiResponse<ModVersionSubmissionComment>)),
        (status = 400, description = "Bad request - locked submission or empty comment"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Mod, version, or submission not found"),
    ),
    security(("bearer_token" = []))
)]
#[post("v1/mods/{id}/versions/{version}/submission/comments")]
pub async fn create_comment(
    path: web::Path<SubmissionPath>,
    data: web::Data<AppData>,
    payload: web::Json<CreateCommentPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;

    let comment_text = payload.comment.trim().to_string();
    if comment_text.is_empty() {
        return Err(ApiError::BadRequest("Comment must not be empty".into()));
    }

    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    let version_id = resolve_version_id(&path.id, &path.version, &mut pool).await?;

    let submission = mod_version_submissions::get_for_mod_version(version_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Submission not found".into()))?;

    if submission.locked {
        return Err(ApiError::BadRequest(
            "Submission is locked; no new comments allowed".into(),
        ));
    }

    // Only the mod developers (or admins) may comment
    if !dev.admin && !developers::has_access_to_mod(dev.id, &path.id, &mut pool).await? {
        return Err(ApiError::Authorization);
    }

    let row = mod_version_submissions::create_comment(version_id, dev.id, &comment_text, &mut pool)
        .await?;

    Ok(HttpResponse::Created().json(ApiResponse {
        error: "".into(),
        payload: row.into_comment(dev),
    }))
}

/// Update a comment on a mod version submission
#[utoipa::path(
    put,
    path = "/v1/mods/{id}/versions/{version}/submission/comments/{comment_id}",
    tag = "mod_version_submissions",
    params(CommentPath),
    request_body = UpdateCommentPayload,
    responses(
        (status = 200, description = "Comment updated", body = inline(ApiResponse<ModVersionSubmissionComment>)),
        (status = 400, description = "Bad request – locked submission or empty comment"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden – may only edit own comments (admins can edit any)"),
        (status = 404, description = "Mod, version, submission, or comment not found"),
    ),
    security(("bearer_token" = []))
)]
#[put("v1/mods/{id}/versions/{version}/submission/comments/{comment_id}")]
pub async fn update_comment(
    path: web::Path<CommentPath>,
    data: web::Data<AppData>,
    payload: web::Json<UpdateCommentPayload>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;

    let comment_text = payload.comment.trim().to_string();
    if comment_text.is_empty() {
        return Err(ApiError::BadRequest("Comment must not be empty".into()));
    }

    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    let version_id = resolve_version_id(&path.id, &path.version, &mut pool).await?;

    let submission = mod_version_submissions::get_for_mod_version(version_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Submission not found".into()))?;

    if submission.locked {
        return Err(ApiError::BadRequest(
            "Submission is locked; comments cannot be edited".into(),
        ));
    }

    let comment_row = mod_version_submissions::get_comment(path.comment_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Comment {} not found", path.comment_id)))?;

    if comment_row.submission_id != version_id {
        return Err(ApiError::NotFound(format!(
            "Comment {} does not belong to this submission",
            path.comment_id
        )));
    }

    if !dev.admin && comment_row.author_id != dev.id {
        return Err(ApiError::Authorization);
    }

    let updated_row =
        mod_version_submissions::update_comment(path.comment_id, &comment_text, &mut pool).await?;

    let author = developers::get_one(updated_row.author_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::InternalError("Author not found".into()))?;

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: updated_row.into_comment(author),
    }))
}

// ── DELETE comment ────────────────────────────────────────────────────────────

/// Delete a comment on a mod version submission
#[utoipa::path(
    delete,
    path = "/v1/mods/{id}/versions/{version}/submission/comments/{comment_id}",
    tag = "mod_version_submissions",
    params(CommentPath),
    responses(
        (status = 204, description = "Comment deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden – may only delete own comments (admins can delete any)"),
        (status = 404, description = "Mod, version, submission, or comment not found"),
    ),
    security(("bearer_token" = []))
)]
#[delete("v1/mods/{id}/versions/{version}/submission/comments/{comment_id}")]
pub async fn delete_comment(
    path: web::Path<CommentPath>,
    data: web::Data<AppData>,
    auth: Auth,
) -> Result<impl Responder, ApiError> {
    let dev = auth.developer()?;

    let mut pool = data.db().acquire().await?;

    if !mods::exists(&path.id, &mut pool).await? {
        return Err(ApiError::NotFound(format!("Mod {} not found", path.id)));
    }

    let version_id = resolve_version_id(&path.id, &path.version, &mut pool).await?;

    let submission = mod_version_submissions::get_for_mod_version(version_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Submission not found".into()))?;

    if submission.locked && !dev.admin {
        return Err(ApiError::BadRequest(
            "Submission is locked; comments cannot be deleted".into(),
        ));
    }

    let comment_row = mod_version_submissions::get_comment(path.comment_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Comment {} not found", path.comment_id)))?;

    if comment_row.submission_id != version_id {
        return Err(ApiError::NotFound(format!(
            "Comment {} does not belong to this submission",
            path.comment_id
        )));
    }

    if !dev.admin && comment_row.author_id != dev.id {
        return Err(ApiError::Authorization);
    }

    mod_version_submissions::delete_comment(path.comment_id, &mut pool).await?;

    Ok(HttpResponse::NoContent())
}
