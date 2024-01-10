-- Add down migration script here

ALTER TABLE mods ALTER COLUMN repository SET NOT NULL;