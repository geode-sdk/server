-- Add down migration script here

DROP TABLE IF EXISTS mod_feedback;
DROP INDEX IF EXISTS public.idx_mod_feedback_mod_version_id;
DROP INDEX IF EXISTS public.idx_mod_feedback_reviewer_id;