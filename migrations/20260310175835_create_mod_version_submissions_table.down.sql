-- Add down migration script here

DROP TABLE IF EXISTS mod_version_submission_comment_audit;
DROP TABLE IF EXISTS mod_version_submission_comments;
DROP TABLE IF EXISTS mod_version_submissions_audit;
DROP TABLE IF EXISTS mod_version_submissions;
DROP TYPE IF EXISTS audit_action;
DROP TYPE IF EXISTS submission_lock;