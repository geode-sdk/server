use std::cmp::PartialEq;
use serde::{Serialize, Deserialize};
use sqlx::{PgConnection,FromRow};

use crate::types::api::ApiError;
use crate::types::models::mod_version::ModVersion;

#[derive(FromRow)]
struct ModFeedbackRow {
    id: i32,
    reviewer_id: i32,
    reviewer_name: String,
    reviewer_admin: bool,
    feedback_type: FeedbackTypeEnum,
    feedback: String,
    decision: bool,
}

#[derive(Serialize)]
pub struct ModFeedback {
    pub score: i32,
    pub mod_id: String,
    pub mod_version: String,
    pub feedback: Vec<ModFeedbackOne>,
}

#[derive(Serialize)]
pub struct Reviewer {
    pub id: i32,
    pub display_name: String,
    pub admin: bool,
}

#[derive(Serialize)]
pub struct ModFeedbackOne {
    #[serde(skip_serializing)]
    pub id: i32,
    pub reviewer: Reviewer,
    pub feedback_type: FeedbackTypeEnum,
    pub feedback: String,
    pub decision: bool,
}

#[derive(sqlx::Type, Serialize, Deserialize, Clone, PartialEq)]
#[sqlx(type_name = "feedback_type")]
pub enum FeedbackTypeEnum {
    Positive,
    Negative,
    Suggestion,
    Note
}

impl ModFeedback {
    pub async fn get_for_mod_version_id(
        version: &ModVersion,
        note_only: bool,
        pool: &mut PgConnection,
    ) -> Result<ModFeedback, ApiError> {
        let result = match sqlx::query_as!(
            ModFeedbackRow,
            r#"SELECT mf.id, mf.reviewer_id, dev.display_name AS reviewer_name, dev.admin AS reviewer_admin, mf.type AS "feedback_type: _", mf.feedback, mf.decision
            FROM mod_feedback mf
			INNER JOIN developers dev ON dev.id = mf.reviewer_id
            WHERE mf.mod_version_id = $1"#,
            version.id
        )
        .fetch_all(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(r) => r,
        };

        let feedback: Vec<ModFeedbackOne> = result.into_iter().filter_map(|row| {
            if note_only && row.feedback_type != FeedbackTypeEnum::Note {
                None
            } else {
                Some(ModFeedbackOne {
                    id: row.id,
                    reviewer: Reviewer {
                        id: row.reviewer_id,
                        display_name: row.reviewer_name,
                        admin: row.reviewer_admin,
                    },
                    feedback_type: row.feedback_type,
                    feedback: row.feedback,
                    decision: row.decision,
                })
            }
        }).collect();

        let positive = feedback.iter().filter(|r| r.feedback_type == FeedbackTypeEnum::Positive).count() as i32;
        let negative = feedback.iter().filter(|r| r.feedback_type == FeedbackTypeEnum::Negative).count() as i32;
        let return_res =
            ModFeedback {
                score: positive - negative,
                mod_id: version.mod_id.clone(),
                mod_version: version.version.clone(),
                feedback,
            };

        Ok(return_res)
    }

    pub async fn set(
        version: &ModVersion,
        reviewer_id: i32,
        feedback_type: FeedbackTypeEnum,
        feedback: &str,
        decision: bool,
        pool: &mut PgConnection
    ) -> Result<(), ApiError> {
        sqlx::query!(
            r#"INSERT INTO mod_feedback (mod_version_id, reviewer_id, type, feedback, decision)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (mod_version_id, reviewer_id)
            DO UPDATE SET type = EXCLUDED.type, feedback = EXCLUDED.feedback, decision = EXCLUDED.decision"#,
            version.id,
            reviewer_id,
            feedback_type as _,
            feedback,
            decision
        )
        .execute(&mut *pool)
        .await
        .map_err(|e| {
            log::error!("{}", e);
            ApiError::DbError
        })?;

        Ok(())
    }
}