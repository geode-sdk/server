use crate::types::models::developer::Developer;
use crate::types::serde::chrono_dt_secs;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash, ToSchema)]
#[sqlx(type_name = "submission_lock", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ModVersionSubmissionLock {
    None,
    Internal,
    Locked,
}

#[derive(Serialize, ToSchema, Debug, Clone)]
pub struct ModVersionSubmission {
    pub mod_version_id: i32,
    pub lock: ModVersionSubmissionLock,
    pub locked_by: Option<Developer>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ModVersionSubmissionRow {
    pub mod_version_id: i32,
    pub lock: ModVersionSubmissionLock,
    pub locked_by: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ModVersionSubmissionRow {
    pub fn into_submission(self, locked_by: Option<Developer>) -> ModVersionSubmission {
        ModVersionSubmission {
            mod_version_id: self.mod_version_id,
            lock: self.lock,
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
    pub attachments: Vec<String>,
    #[serde(with = "chrono_dt_secs")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono_dt_secs::option")]
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
    pub fn into_comment(
        self,
        author: Developer,
        attachment_links: Vec<String>,
    ) -> ModVersionSubmissionComment {
        ModVersionSubmissionComment {
            id: self.id,
            submission_id: self.submission_id,
            comment: self.comment,
            author,
            attachments: attachment_links,
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
    #[serde(with = "chrono_dt_secs")]
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
    pub lock: ModVersionSubmissionLock,
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
