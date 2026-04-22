use crate::database::DatabaseError;
use crate::types::models::audit_actions::{AuditAction, AuditActionRow};
use crate::types::models::mod_version_submission::{
    ModVersionSubmissionAttachmentRow, ModVersionSubmissionCommentRow, ModVersionSubmissionLock, ModVersionSubmissionRow
};
use sqlx::{Error, PgConnection};
use std::collections::HashMap;

pub async fn get_for_mod_version(
    id: i32,
    conn: &mut PgConnection,
) -> Result<Option<ModVersionSubmissionRow>, DatabaseError> {
    sqlx::query_as!(
        ModVersionSubmissionRow,
        r#"SELECT
        mod_version_id, lock as "lock: _", locked_by,
        created_at, updated_at
        FROM mod_version_submissions
        WHERE mod_version_id = $1"#,
        id
    )
    .fetch_optional(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::get_for_mod_versions failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn get_audit_for_submission(
    id: i32,
    conn: &mut PgConnection,
) -> Result<Vec<AuditActionRow>, DatabaseError> {
    sqlx::query_as!(
        AuditActionRow,
        r#"SELECT
            action as "action: _", details, performed_by, performed_at
        FROM mod_version_submissions_audit
        WHERE submission_id = $1"#,
        id
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::get_audit_for_submission failed: {e}",))
    .map_err(|e| e.into())
}

pub async fn create(
    mod_version_id: i32,
    conn: &mut PgConnection,
) -> Result<ModVersionSubmissionRow, DatabaseError> {
    let row = sqlx::query_as!(
        ModVersionSubmissionRow,
        r#"INSERT INTO mod_version_submissions (mod_version_id)
        VALUES ($1)
        RETURNING mod_version_id, lock as "lock: _", locked_by, created_at, updated_at"#,
        mod_version_id
    )
    .fetch_one(&mut *conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::create failed: {e}"))?;

    insert_submission_audit(mod_version_id, AuditAction::Created, None, None, conn).await?;
    Ok(row)
}

pub async fn set_locked(
    mod_version_id: i32,
    lock: ModVersionSubmissionLock,
    locked_by: Option<i32>,
    conn: &mut PgConnection,
) -> Result<ModVersionSubmissionRow, DatabaseError> {
    insert_submission_audit(
        mod_version_id,
        AuditAction::Updated,
        Some(&format!(
            "Submission {}{}",
            match lock {
                ModVersionSubmissionLock::None => "unlocked",
                ModVersionSubmissionLock::Internal => "restricted to mod developers and admins",
                ModVersionSubmissionLock::Locked => "locked"
            },
            if locked_by.is_none() {
                " automatically"
            } else {
                ""
            }
        )),
        locked_by,
        &mut *conn,
    )
    .await?;

    sqlx::query_as!(
        ModVersionSubmissionRow,
        r#"UPDATE mod_version_submissions
        SET lock = $1, locked_by = $2, updated_at = NOW()
        WHERE mod_version_id = $3
        RETURNING mod_version_id, lock as "lock: _", locked_by, created_at, updated_at"#,
        lock as ModVersionSubmissionLock,
        locked_by,
        mod_version_id
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::set_locked failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn get_paginated_comments_for_submission(
    id: i32,
    page: i64,
    per_page: i64,
    conn: &mut PgConnection,
) -> Result<Vec<ModVersionSubmissionCommentRow>, DatabaseError> {
    sqlx::query_as!(
        ModVersionSubmissionCommentRow,
        "SELECT
            id, submission_id, comment, author_id,
            created_at, updated_at
        FROM mod_version_submission_comments
        WHERE submission_id = $1
        ORDER BY id DESC
        LIMIT $2 OFFSET $3",
        id,
        per_page,
        (page - 1) * per_page
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| {
        log::error!("mod_version_submissions::get_paginated_items_for_submission failed: {e}")
    })
    .map_err(|e| e.into())
}

pub async fn count_comments_for_submission(
    id: i32,
    conn: &mut PgConnection,
) -> Result<i64, DatabaseError> {
    sqlx::query_scalar!(
        "SELECT COUNT(*) FROM mod_version_submission_comments WHERE submission_id = $1",
        id
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| {
        log::error!("mod_version_submissions::count_comments_for_submission failed: {e}")
    })
    .map(|c| c.unwrap_or(0))
    .map_err(|e| e.into())
}

pub async fn create_comment(
    submission_id: i32,
    author_id: i32,
    comment: &str,
    conn: &mut PgConnection,
) -> Result<ModVersionSubmissionCommentRow, DatabaseError> {
    sqlx::query_as!(
        ModVersionSubmissionCommentRow,
        "INSERT INTO mod_version_submission_comments (submission_id, author_id, comment)
        VALUES ($1, $2, $3)
        RETURNING id, submission_id, comment, author_id, created_at, updated_at",
        submission_id,
        author_id,
        comment
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::create_comment failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn get_comment(
    comment_id: i64,
    conn: &mut PgConnection,
) -> Result<Option<ModVersionSubmissionCommentRow>, DatabaseError> {
    sqlx::query_as!(
        ModVersionSubmissionCommentRow,
        "SELECT id, submission_id, comment, author_id, created_at, updated_at
        FROM mod_version_submission_comments
        WHERE id = $1",
        comment_id
    )
    .fetch_optional(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::get_comment failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn update_comment(
    comment_id: i64,
    new_text: &str,
    conn: &mut PgConnection,
) -> Result<ModVersionSubmissionCommentRow, DatabaseError> {
    sqlx::query_as!(
        ModVersionSubmissionCommentRow,
        "UPDATE mod_version_submission_comments
        SET comment = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, submission_id, comment, author_id, created_at, updated_at",
        new_text,
        comment_id
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::update_comment failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn delete_comment(
    comment_id: i64,
    conn: &mut PgConnection,
) -> Result<bool, DatabaseError> {
    let result = sqlx::query!(
        "DELETE FROM mod_version_submission_comments WHERE id = $1",
        comment_id
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::delete_comment failed: {e}"))?;
    Ok(result.rows_affected() > 0)
}

pub async fn get_audit_for_comment(
    id: i64,
    conn: &mut PgConnection,
) -> Result<Vec<AuditActionRow>, DatabaseError> {
    sqlx::query_as!(
        AuditActionRow,
        r#"SELECT
            action as "action: _", details, performed_by, performed_at
        FROM mod_version_submission_comment_audit
        WHERE comment_id = $1"#,
        id
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::get_audit_for_comment failed: {e}",))
    .map_err(|e| e.into())
}

pub async fn count_attachments_for_comment(
    comment_id: i64,
    conn: &mut PgConnection,
) -> Result<i64, DatabaseError> {
    sqlx::query_scalar!(
        "SELECT COUNT(*) FROM mod_version_submission_comment_attachments WHERE comment_id = $1",
        comment_id
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| {
        log::error!("mod_version_submissions::count_attachments_for_comment failed: {e}")
    })
    .map(|c| c.unwrap_or(0))
    .map_err(|e| e.into())
}

pub async fn create_attachment(
    comment_id: i64,
    filename: &str,
    conn: &mut PgConnection,
) -> Result<ModVersionSubmissionAttachmentRow, DatabaseError> {
    sqlx::query_as!(
        ModVersionSubmissionAttachmentRow,
        "INSERT INTO mod_version_submission_comment_attachments (comment_id, filename)
        VALUES ($1, $2)
        RETURNING id, comment_id, filename, created_at",
        comment_id,
        filename
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::create_attachment failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn get_attachments_for_comment(
    comment_id: i64,
    conn: &mut PgConnection,
) -> Result<Vec<ModVersionSubmissionAttachmentRow>, DatabaseError> {
    sqlx::query_as!(
        ModVersionSubmissionAttachmentRow,
        "SELECT id, comment_id, filename, created_at
        FROM mod_version_submission_comment_attachments
        WHERE comment_id = $1
        ORDER BY id ASC",
        comment_id
    )
    .fetch_all(conn)
    .await
    .inspect_err(|e| {
        log::error!("mod_version_submissions::get_attachments_for_comment failed: {e}")
    })
    .map_err(|e: Error| e.into())
}

pub async fn get_attachment(
    attachment_id: i64,
    conn: &mut PgConnection,
) -> Result<Option<ModVersionSubmissionAttachmentRow>, DatabaseError> {
    sqlx::query_as!(
        ModVersionSubmissionAttachmentRow,
        "SELECT id, comment_id, filename, created_at
        FROM mod_version_submission_comment_attachments
        WHERE id = $1",
        attachment_id
    )
    .fetch_optional(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::get_attachment failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn delete_attachment(
    attachment_id: i64,
    conn: &mut PgConnection,
) -> Result<bool, DatabaseError> {
    let result = sqlx::query!(
        "DELETE FROM mod_version_submission_comment_attachments WHERE id = $1",
        attachment_id
    )
    .execute(conn)
    .await
    .inspect_err(|e| log::error!("mod_version_submissions::delete_attachment failed: {e}"))?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_attachments_for_comment(
    comment_id: i64,
    conn: &mut PgConnection,
) -> Result<bool, DatabaseError> {
    let result = sqlx::query!(
        "DELETE FROM mod_version_submission_comment_attachments WHERE comment_id = $1",
        comment_id
    )
    .execute(conn)
    .await
    .inspect_err(|e| {
        log::error!("mod_version_submissions::delete_attachments_for_comment_id failed: {e}")
    })?;
    Ok(result.rows_affected() > 0)
}

pub async fn count_references_to_filename(
    filename: &str,
    conn: &mut PgConnection,
) -> Result<i64, DatabaseError> {
    sqlx::query_scalar!(
        "SELECT COUNT(*) FROM mod_version_submission_comment_attachments WHERE filename = $1",
        filename
    )
    .fetch_one(conn)
    .await
    .inspect_err(|e| {
        log::error!("mod_version_submissions::count_references_to_filename failed: {e}")
    })
    .map(|c| c.unwrap_or(0))
    .map_err(|e| e.into())
}

pub async fn count_references_to_filenames(
    filenames: &[String],
    conn: &mut PgConnection,
) -> Result<HashMap<String, i64>, DatabaseError> {
    sqlx::query!(
        "SELECT
            filename, COUNT(*) as count
        FROM mod_version_submission_comment_attachments
        WHERE filename = ANY($1)
        GROUP BY filename",
        filenames
    )
    .fetch_all(conn)
    .await
    .map(|x| {
        x.into_iter()
            .map(|record| (record.filename, record.count.unwrap_or(0)))
            .collect()
    })
    .inspect_err(|e| {
        log::error!("mod_version_submissions::count_references_to_filenames failed: {e}")
    })
    .map_err(|e| e.into())
}

pub async fn insert_submission_audit(
    id: i32,
    action: AuditAction,
    details: Option<&str>,
    performed_by: Option<i32>,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query!(
        "INSERT INTO mod_version_submissions_audit (submission_id, action, details, performed_by)
        VALUES ($1, $2, $3, $4)",
        id,
        action as AuditAction,
        details,
        performed_by
    )
    .execute(conn)
    .await
    .map(|_| ())
    .inspect_err(|e| log::error!("mod_version_submissions::insert_submission_audit failed: {e}"))
    .map_err(|e| e.into())
}

pub async fn insert_comment_audit(
    id: i64,
    action: AuditAction,
    details: Option<&str>,
    performed_by: Option<i32>,
    conn: &mut PgConnection,
) -> Result<(), DatabaseError> {
    sqlx::query!(
        "INSERT INTO mod_version_submission_comment_audit (comment_id, action, details, performed_by)\
        VALUES ($1, $2, $3, $4)",
        id,
        action as AuditAction,
        details,
        performed_by
    )
        .execute(conn)
        .await
        .map(|_| ())
        .inspect_err(|e| log::error!("mod_version_submissions::insert_comment_audit failed: {e}"))
        .map_err(|e| e.into())
}
