-- Add down migration script here

ALTER TABLE auth_tokens DROP COLUMN IF EXISTS expires_at;
DROP TABLE IF EXISTS refresh_tokens;