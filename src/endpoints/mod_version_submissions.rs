use super::ApiError;
use crate::config::AppData;
use crate::database::repository::{developers, mod_version_submissions, mod_versions, mods};
use crate::extractors::auth::Auth;
use crate::types::api::{ApiResponse, PaginatedData};
use crate::types::models::mod_version_submission::{
    CreateCommentPayload, ModVersionSubmission, ModVersionSubmissionAttachment,
    ModVersionSubmissionComment, UpdateCommentPayload, UpdateSubmissionPayload,
};
use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use futures::StreamExt;
use serde::Deserialize;
use sqlx::PgConnection;
use std::collections::HashMap;
use utoipa::IntoParams;

fn sanitize_comment(raw: &str) -> String {
    ammonia::Builder::default()
        .tags(std::collections::HashSet::new())
        .clean(raw)
        .to_string()
        .trim()
        .to_string()
}

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
struct AttachmentPath {
    id: String,
    version: String,
    comment_id: i64,
    attachment_id: i64,
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

    let locked_by_id = if payload.locked { Some(dev.id) } else { None };

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

    let authors_map = developers::get_many_by_id(&author_ids, &mut pool)
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

    let comment_text = sanitize_comment(&payload.comment);
    if comment_text.is_empty() {
        return Err(ApiError::BadRequest("Comment must not be empty".into()));
    }
    if comment_text.len() > 1000 {
        return Err(ApiError::BadRequest(
            "Comment must not exceed 1000 characters".into(),
        ));
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

    let comment_text = sanitize_comment(&payload.comment);
    if comment_text.is_empty() {
        return Err(ApiError::BadRequest("Comment must not be empty".into()));
    }
    if comment_text.len() > 1000 {
        return Err(ApiError::BadRequest(
            "Comment must not exceed 1000 characters".into(),
        ));
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

/// List attachments for a submission comment
#[utoipa::path(
    get,
    path = "/v1/mods/{id}/versions/{version}/submission/comments/{comment_id}/attachments",
    tag = "mod_version_submissions",
    params(CommentPath),
    responses(
        (status = 200, description = "List of attachments", body = inline(ApiResponse<Vec<ModVersionSubmissionAttachment>>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Mod, version, submission, or comment not found"),
    ),
    security(("bearer_token" = []))
)]
#[get("v1/mods/{id}/versions/{version}/submission/comments/{comment_id}/attachments")]
pub async fn get_attachments(
    path: web::Path<CommentPath>,
    data: web::Data<AppData>,
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

    let comment_row = mod_version_submissions::get_comment(path.comment_id, &mut pool)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Comment {} not found", path.comment_id)))?;

    if comment_row.submission_id != version_id {
        return Err(ApiError::NotFound(format!(
            "Comment {} does not belong to this submission",
            path.comment_id
        )));
    }

    let rows =
        mod_version_submissions::get_attachments_for_comment(path.comment_id, &mut pool).await?;

    let app_url = data.app_url().to_string();
    let attachments: Vec<ModVersionSubmissionAttachment> = rows
        .into_iter()
        .map(|r| r.into_attachment(&app_url))
        .collect();

    Ok(web::Json(ApiResponse {
        error: "".into(),
        payload: attachments,
    }))
}

/// Upload attachments to a submission comment
#[utoipa::path(
    post,
    path = "/v1/mods/{id}/versions/{version}/submission/comments/{comment_id}/attachments",
    tag = "mod_version_submissions",
    params(CommentPath),
    responses(
        (status = 201, description = "Attachments uploaded", body = inline(ApiResponse<Vec<ModVersionSubmissionAttachment>>)),
        (status = 400, description = "Bad request - no images, file too large, or attachment limit exceeded"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Mod, version, submission, or comment not found"),
    ),
    security(("bearer_token" = []))
)]
#[post("v1/mods/{id}/versions/{version}/submission/comments/{comment_id}/attachments")]
pub async fn upload_attachments(
    path: web::Path<CommentPath>,
    data: web::Data<AppData>,
    mut multipart: Multipart,
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

    if submission.locked {
        return Err(ApiError::BadRequest(
            "Submission is locked; attachments cannot be uploaded".into(),
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

    if comment_row.author_id != dev.id {
        return Err(ApiError::Authorization);
    }

    // Collect all `image` fields from the multipart stream
    const MAX_BYTES: usize = 5 * 1024 * 1024;
    let mut images: Vec<bytes::Bytes> = Vec::new();
    while let Some(field) = multipart.next().await {
        let mut field = field.map_err(|e| ApiError::BadRequest(e.to_string()))?;
        if field.name() != Some("image") {
            continue;
        }
        let mut buf = bytes::BytesMut::new();
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(|e| ApiError::BadRequest(e.to_string()))?;
            buf.extend_from_slice(&chunk);
            if buf.len() > MAX_BYTES {
                return Err(ApiError::BadRequest("Image exceeds 5 MB limit".into()));
            }
        }
        images.push(buf.freeze());
    }

    if images.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one image field is required".into(),
        ));
    }

    let existing =
        mod_version_submissions::count_attachments_for_comment(path.comment_id, &mut pool).await?;
    if existing + images.len() as i64 > 5 {
        return Err(ApiError::BadRequest(format!(
            "Comment already has {} attachment(s); adding {} would exceed the limit of 5",
            existing,
            images.len()
        )));
    }

    let storage_path = data.storage_path().to_string();

    // Decode → encode WebP → hash, all in a blocking thread
    let processed: Vec<(String, Vec<u8>)> =
        tokio::task::spawn_blocking(move || -> Result<Vec<(String, Vec<u8>)>, ApiError> {
            images
                .into_iter()
                .map(|raw| {
                    let img = image::load_from_memory(&raw)
                        .map_err(|e| ApiError::BadRequest(format!("Invalid image: {e}")))?;
                    let mut webp_bytes: Vec<u8> = Vec::new();
                    img.write_to(
                        &mut std::io::Cursor::new(&mut webp_bytes),
                        image::ImageFormat::WebP,
                    )
                    .map_err(|e| ApiError::InternalError(format!("WebP encode failed: {e}")))?;
                    let filename = format!("{}.webp", sha256::digest(&webp_bytes));
                    Ok((filename, webp_bytes))
                })
                .collect()
        })
        .await
        .map_err(|e| ApiError::InternalError(format!("Task join error: {e}")))??;

    // Write files and insert DB rows
    let attachments_dir = format!("{}/submission_attachments", storage_path);
    let app_url = data.app_url().to_string();
    let mut result = Vec::with_capacity(processed.len());
    for (filename, webp_bytes) in processed {
        let file_path = format!("{}/{}", attachments_dir, filename);
        if !std::path::Path::new(&file_path).exists() {
            tokio::fs::write(&file_path, &webp_bytes)
                .await
                .map_err(|e| ApiError::InternalError(format!("Failed to write file: {e}")))?;
        }
        let row = mod_version_submissions::create_attachment(path.comment_id, &filename, &mut pool)
            .await?;
        result.push(row.into_attachment(&app_url));
    }

    Ok(HttpResponse::Created().json(ApiResponse {
        error: "".into(),
        payload: result,
    }))
}

/// Delete an attachment from a submission comment
#[utoipa::path(
    delete,
    path = "/v1/mods/{id}/versions/{version}/submission/comments/{comment_id}/attachments/{attachment_id}",
    tag = "mod_version_submissions",
    params(AttachmentPath),
    responses(
        (status = 204, description = "Attachment deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Mod, version, submission, comment, or attachment not found"),
    ),
    security(("bearer_token" = []))
)]
#[delete(
    "v1/mods/{id}/versions/{version}/submission/comments/{comment_id}/attachments/{attachment_id}"
)]
pub async fn delete_attachment(
    path: web::Path<AttachmentPath>,
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
            "Submission is locked; attachments cannot be deleted".into(),
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

    let attachment = mod_version_submissions::get_attachment(path.attachment_id, &mut pool)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Attachment {} not found", path.attachment_id))
        })?;

    if attachment.comment_id != path.comment_id {
        return Err(ApiError::NotFound(format!(
            "Attachment {} does not belong to this comment",
            path.attachment_id
        )));
    }

    let filename = attachment.filename.clone();
    mod_version_submissions::delete_attachment(path.attachment_id, &mut pool).await?;

    let remaining =
        mod_version_submissions::count_references_to_filename(&filename, &mut pool).await?;
    if remaining == 0 {
        let file_path = format!(
            "{}/submission_attachments/{}",
            data.storage_path(),
            filename
        );
        tokio::fs::remove_file(&file_path).await.ok();
    }

    Ok(HttpResponse::NoContent())
}
