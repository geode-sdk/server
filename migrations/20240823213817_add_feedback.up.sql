-- Add up migration script here

CREATE TYPE feedback_type AS ENUM
    ('Positive', 'Negative', 'Suggestion', 'Note');

CREATE TABLE mod_feedback
(
    id SERIAL PRIMARY KEY NOT NULL,
    mod_version_id INTEGER NOT NULL,
    reviewer_id INTEGER NOT NULL,
    feedback TEXT COLLATE pg_catalog."default" NOT NULL,
    decision BOOLEAN NOT NULL DEFAULT false,
    type feedback_type NOT NULL,
    dev bool NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT mod_feedback_mod_version_id_fkey FOREIGN KEY (mod_version_id)
        REFERENCES public.mod_versions (id)
        ON DELETE CASCADE,
    CONSTRAINT mod_feedback_reviewer_id_fkey FOREIGN KEY (reviewer_id)
        REFERENCES public.developers (id)
        ON DELETE CASCADE
);

CREATE INDEX idx_mod_feedback_mod_version_id
    ON public.mod_feedback (mod_version_id);

CREATE INDEX idx_mod_feedback_reviewer_id
    ON public.mod_feedback (reviewer_id);