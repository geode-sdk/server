-- Add down migration script here

ALTER TABLE mod_versions
ADD COLUMN geode TEXT;

DROP INDEX idx_mod_versions_geode_major_minor;
DROP INDEX idx_mod_versions_geode_patch;
DROP INDEX idx_mod_versions_geode_meta;

UPDATE mod_versions mv
SET geode = CASE
WHEN mv.geode_meta IS NOT NULL
    THEN mv.geode_major || '.' || mv.geode_minor || '.' || mv.geode_patch || '-' || mv.geode_meta
ELSE mv.geode_major || '.' || mv.geode_minor || '.' || mv.geode_patch
END;

ALTER TABLE mod_versions
ALTER COLUMN geode SET NOT NULL;

ALTER TABLE mod_versions
DROP COLUMN geode_major;
ALTER TABLE mod_versions
DROP COLUMN geode_minor;
ALTER TABLE mod_versions
DROP COLUMN geode_patch;
ALTER TABLE mod_versions
DROP COLUMN geode_meta;