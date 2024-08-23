use serde::Serialize;
use sqlx::{PgConnection};

use crate::types::api::ApiError;
use crate::types::models::mod_version::ModVersion;

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
    pub positive: bool,
    pub feedback: String,
    pub decision: bool,
}

impl ModFeedback {
    pub async fn get_for_mod_version_id(
        version: &ModVersion,
        pool: &mut PgConnection,
    )-> Result<ModFeedback, ApiError> {
        let result = match sqlx::query!(
            r#"SELECT mf.id, mf.reviewer_id, dev.display_name AS reviewer_name, dev.admin AS reviewer_admin, mf.positive, mf.feedback, mf.decision
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

        let feedback: Vec<ModFeedbackOne> = result.iter().map(|row| {
            ModFeedbackOne {
                id: row.id,
                reviewer: Reviewer {
                    id: row.reviewer_id,
                    display_name: row.reviewer_name.clone(),
                    admin: row.reviewer_admin,
                },
                positive: row.positive,
                feedback: row.feedback.clone(),
                decision: row.decision,
            }
        }).collect();

        let positive = result.iter().filter(|r| r.positive).count() as i32;
        let negative = result.iter().filter(|r| !r.positive).count() as i32;
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
        positive: bool,
        feedback: &str,
        decision: bool,
        pool: &mut PgConnection
    ) -> Result<(), ApiError> {
        sqlx::query!(
            r#"INSERT INTO mod_feedback (mod_version_id, reviewer_id, positive, feedback, decision)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (mod_version_id, reviewer_id)
            DO UPDATE SET positive = EXCLUDED.positive, feedback = EXCLUDED.feedback, decision = EXCLUDED.decision"#,
            version.id,
            reviewer_id,
            positive,
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