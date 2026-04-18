-- Add down migration script here
ALTER TABLE developers DROP COLUMN IF EXISTS note;
DROP TABLE IF EXISTS bans;