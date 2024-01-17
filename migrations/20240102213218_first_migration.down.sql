-- Add down migration script here

DROP TABLE IF EXISTS mods_mod_tags;
DROP TABLE IF EXISTS mod_gd_versions;
DROP TABLE IF EXISTS dependencies;
DROP TABLE IF EXISTS incompatibilities;
DROP TABLE IF EXISTS mod_tags;
DROP TABLE IF EXISTS mod_versions;
DROP TABLE IF EXISTS mods_developers;
DROP TABLE IF EXISTS dependencies;
DROP TABLE IF EXISTS mods;
DROP TABLE IF EXISTS developers;

DROP INDEX IF EXISTS idx_version_id;

DROP TYPE IF EXISTS dependency_importance;
DROP TYPE IF EXISTS version_compare;
DROP TYPE IF EXISTS gd_version;
DROP TYPE IF EXISTS gd_ver_platform;