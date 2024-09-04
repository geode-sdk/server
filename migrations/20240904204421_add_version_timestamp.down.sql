-- Add down migration script here

ALTER TABLE mod_versions DROP column IF EXISTS created_at;
ALTER TABLE mod_versions DROP column IF EXISTS updated_at;
