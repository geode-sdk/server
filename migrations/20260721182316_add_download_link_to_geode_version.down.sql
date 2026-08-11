DROP TABLE IF EXISTS geode_version_download;

ALTER TABLE geode_versions
    DROP COLUMN resources_url,
    DROP COLUMN resources_hash;
