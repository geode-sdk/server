-- Add up migration script here

CREATE TYPE feedback_type AS ENUM
    ('Positive', 'Negative', 'Suggestion', 'Note');

ALTER TABLE mod_feedback
    DROP COLUMN positive,
    ADD COLUMN type feedback_type NOT NULL;