-- Add down migration script here
DELETE FROM mod_tags
WHERE name IN ('joke', 'api');