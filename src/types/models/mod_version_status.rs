use serde::{Deserialize, Serialize};

#[derive(sqlx::Type, Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase", type_name = "mod_version_status")]
pub enum ModVersionStatusEnum {
    Pending,
    Accepted,
    Rejected,
    Unlisted,
}

impl ModVersionStatusEnum {
    pub fn get_unlisted_mod_filter_for_array(statuses: &[ModVersionStatusEnum]) -> Option<bool> {
        let first = statuses.first();

        if statuses.len() == 1 && first.is_some_and(|x| *x== ModVersionStatusEnum::Unlisted) {
            Some(true)
        } else {
            let found = statuses.iter().find(|y| **y == ModVersionStatusEnum::Unlisted).is_some();

            if found {
                None
            } else {
                Some(false)
            }
        }
    }
}