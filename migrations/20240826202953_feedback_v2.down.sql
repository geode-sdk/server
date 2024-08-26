-- Add down migration script here

DROP TYPE feedback_type;

ALTER TABLE mod_feedback
    ADD COLUMN positive BOOLEAN NOT NULL,
    DROP COLUMN type;