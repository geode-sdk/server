-- Add up migration script here

CREATE TABLE mod_feedback
(
    id SERIAL PRIMARY KEY NOT NULL,
    mod_version_id INTEGER NOT NULL,
    reviewer_id INTEGER NOT NULL,
    positive BOOLEAN NOT NULL,
    feedback TEXT COLLATE pg_catalog."default" NOT NULL DEFAULT 'No feedback provided.'::text,
    decision BOOLEAN NOT NULL DEFAULT false,
    CONSTRAINT mod_feedback_mod_id_reviewer_id_key UNIQUE (mod_version_id, reviewer_id),
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