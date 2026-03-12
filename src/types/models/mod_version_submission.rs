use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::types::models::developer::Developer;

#[derive(Serialize, ToSchema, Debug, Clone)]
pub struct ModVersionSubmission {
    pub mod_version_id: i32,
    pub locked: bool,
    pub locked_by: Option<Developer>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ModVersionSubmissionRow {
    pub mod_version_id: i32,
    pub locked: bool,
    pub locked_by: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ModVersionSubmissionRow {
    pub fn into_submission(self, locked_by: Option<Developer>) -> ModVersionSubmission {
        ModVersionSubmission {
            mod_version_id: self.mod_version_id,
            locked: self.locked,
            locked_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Serialize, ToSchema, Debug, Clone)]
pub struct ModVersionSubmissionComment {
    pub id: i64,
    pub submission_id: i32,
    pub comment: String,
    pub author: Developer,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

pub struct ModVersionSubmissionCommentRow {
    pub id: i64,
    pub submission_id: i32,
    pub comment: String,
    pub author_id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl ModVersionSubmissionCommentRow {
    pub fn into_comment(self, author: Developer) -> ModVersionSubmissionComment {
        ModVersionSubmissionComment {
            id: self.id,
            submission_id: self.submission_id,
            comment: self.comment,
            author,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Serialize, ToSchema, Debug, Clone)]
pub struct ModVersionSubmissionAttachment {
    pub id: i64,
    pub comment_id: i64,
    pub url: String,
    pub created_at: DateTime<Utc>,
}

pub struct ModVersionSubmissionAttachmentRow {
    pub id: i64,
    pub comment_id: i64,
    pub filename: String,
    pub created_at: DateTime<Utc>,
}

impl ModVersionSubmissionAttachmentRow {
    pub fn into_attachment(self, app_url: &str) -> ModVersionSubmissionAttachment {
        ModVersionSubmissionAttachment {
            id: self.id,
            comment_id: self.comment_id,
            url: format!(
                "{}/storage/submission-attachments/{}",
                app_url, self.filename
            ),
            created_at: self.created_at,
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateSubmissionPayload {
    pub locked: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateCommentPayload {
    #[schema(max_length = 1000)]
    /// Plain text comment; HTML tags are stripped
    pub comment: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateCommentPayload {
    #[schema(max_length = 1000)]
    /// Plain text comment; HTML tags are stripped
    pub comment: String,
}

