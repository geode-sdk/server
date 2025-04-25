-- Add up migration script here
ALTER TABLE mod_versions
ADD COLUMN geode_major INTEGER;
ALTER TABLE mod_versions
ADD COLUMN geode_minor INTEGER;
ALTER TABLE mod_versions
ADD COLUMN geode_patch INTEGER;
ALTER TABLE mod_versions
ADD COLUMN geode_meta TEXT COLLATE en_natural;

UPDATE mod_versions mv
SET geode_major = TRIM('v' FROM SPLIT_PART(mv.geode, '.', 1))::INTEGER,
geode_minor = SPLIT_PART(mv.geode, '.', 2)::INTEGER,
geode_patch = SPLIT_PART(SPLIT_PART(mv.geode, '.', 3), '-', 1)::INTEGER,
geode_meta = SPLIT_PART(mv.geode, '-', 2);

UPDATE mod_versions mv
SET geode_meta = NULLIF(mv.geode_meta, '');

ALTER TABLE mod_versions
ALTER COLUMN geode_major
SET NOT NULL;
ALTER TABLE mod_versions
ALTER COLUMN geode_minor
SET NOT NULL;
ALTER TABLE mod_versions
ALTER COLUMN geode_patch
SET NOT NULL;

ALTER TABLE mod_versions DROP COLUMN geode;

CREATE INDEX idx_mod_versions_geode_major ON mod_versions(geode_major);
CREATE INDEX idx_mod_versions_geode_minor ON mod_versions(geode_major);
CREATE INDEX idx_mod_versions_geode_patch ON mod_versions(geode_patch);
CREATE INDEX idx_mod_versions_geode_meta ON mod_versions(geode_meta);

CREATE OR REPLACE FUNCTION format_semver(
    major int, minor int, patch int, meta text
) RETURNS TEXT AS $$
    SELECT COALESCE(CASE
        WHEN meta IS NULL OR meta = ''
            THEN major || '.' || minor || '.' || patch
        ELSE major || '.' || minor || '.' || patch || '-' || meta
    END, '');
$$ LANGUAGE SQL IMMUTABLE CALLED ON NULL INPUT;
