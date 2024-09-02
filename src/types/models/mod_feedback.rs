use std::cmp::PartialEq;
use serde::{Serialize, Deserialize};
use sqlx::{PgConnection, FromRow, Postgres, QueryBuilder};

use crate::types::api::ApiError;
use crate::types::models::developer::Developer;
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
    dev: bool
}

#[derive(Serialize)]
pub struct ModFeedback {
    pub score: Score,
    pub mod_id: String,
    pub mod_version: String,
    pub feedback: Vec<ModFeedbackOne>,
}

#[derive(Serialize)]
pub struct Reviewer {
    pub id: i32,
    pub dev: bool,
    pub display_name: String,
    pub admin: bool,
}

#[derive(Serialize)]
pub struct ModFeedbackOne {
    pub id: i32,
    pub reviewer: Reviewer,
    pub feedback_type: FeedbackTypeEnum,
    pub feedback: String,
    pub decision: bool,
}

#[derive(Serialize)]
pub struct Score {
    pub score: i32,
    pub positive: i32,
    pub negative: i32,
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
        dev_only: bool,
        pool: &mut PgConnection,
    ) -> Result<ModFeedback, ApiError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT mf.id, mf.reviewer_id, dev.display_name AS reviewer_name, dev.admin AS reviewer_admin, mf.type AS "feedback_type: _", mf.feedback, mf.decision, mf.dev
            FROM mod_feedback mf
			INNER JOIN developers dev ON dev.id = mf.reviewer_id
            WHERE mf.mod_version_id = "#
        );
        query_builder.push_bind(version.id);
        if dev_only {
            query_builder.push(" AND mf.dev = true");
        }
        query_builder.push(" ORDER BY created_at DESC");
        let result = match query_builder
        .build_query_as::<ModFeedbackRow>()
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
            Some(ModFeedbackOne {
                id: row.id,
                reviewer: Reviewer {
                    id: row.reviewer_id,
                    display_name: row.reviewer_name,
                    admin: row.reviewer_admin,
                    dev: row.dev,
                },
                feedback_type: row.feedback_type,
                feedback: row.feedback,
                decision: row.decision,
            })
        }).collect();

        let positive = feedback.iter().filter(|r| r.feedback_type == FeedbackTypeEnum::Positive).count() as i32;
        let negative = feedback.iter().filter(|r| r.feedback_type == FeedbackTypeEnum::Negative).count() as i32;
        let return_res =
            ModFeedback {
                score: Score {
                    score: positive - negative,
                    positive,
                    negative,
                },
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
        dev: bool,
        pool: &mut PgConnection
    ) -> Result<i32, ApiError> {
        let result = sqlx::query!(
            r#"INSERT INTO mod_feedback (mod_version_id, reviewer_id, type, feedback, decision, dev)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id"#,
            version.id,
            reviewer_id,
            feedback_type as _,
            feedback,
            decision,
            dev
        )
        .fetch_one(&mut *pool)
        .await
        .map_err(|e| {
            log::error!("{}", e);
            ApiError::DbError
        })?;

        Ok(result.id)
    }

    pub async fn remove(
        feedback_id: i32,
        pool: &mut PgConnection
    ) -> Result<(), ApiError> {
        sqlx::query!(
            r#"DELETE FROM mod_feedback
            WHERE id = $1"#,
            feedback_id,
        )
        .execute(&mut *pool)
        .await
        .map_err(|e| {
            log::error!("{}", e);
            ApiError::DbError
        })?;

        Ok(())
    }

    pub async fn get_feedback_by_id(
        feedback_id: i32,
        pool: &mut PgConnection
    ) -> Result<ModFeedbackOne, ApiError> {
        let result = match sqlx::query_as!(
            ModFeedbackRow,
            r#"SELECT mf.id, mf.reviewer_id, dev.display_name AS reviewer_name, dev.admin AS reviewer_admin, mf.type AS "feedback_type: _", mf.feedback, mf.decision, mf.dev
            FROM mod_feedback mf
			INNER JOIN developers dev ON dev.id = mf.reviewer_id
            WHERE mf.id = $1"#,
            feedback_id
        )
        .fetch_optional(&mut *pool)
        .await
        {
            Err(e) => {
                log::error!("{}", e);
                return Err(ApiError::DbError);
            }
            Ok(None) => {
                return Err(ApiError::NotFound("Feedback not found".to_string()));
            }
            Ok(Some(r)) => r,
        };

        Ok(ModFeedbackOne {
            id: result.id,
            reviewer: Reviewer {
                id: result.reviewer_id,
                display_name: result.reviewer_name,
                admin: result.reviewer_admin,
                dev: result.dev,
            },
            feedback_type: result.feedback_type,
            feedback: result.feedback,
            decision: result.decision,
        })
    }
}