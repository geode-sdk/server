-- Add down migration script here

DROP TABLE IF EXISTS mod_versions;
DROP TABLE IF EXISTS mods_developers;
DROP TABLE IF EXISTS dependencies;
DROP TABLE IF EXISTS mods;
DROP TABLE IF EXISTS developers;

DROP INDEX IF EXISTS idx_version_id;

DROP TYPE IF EXISTS mod_importance;