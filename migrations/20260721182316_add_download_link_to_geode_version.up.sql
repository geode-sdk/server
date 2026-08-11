CREATE TABLE geode_version_download (
    tag TEXT NOT NULL REFERENCES geode_versions(tag) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    url TEXT NOT NULL,
    hash TEXT NOT NULL,
    PRIMARY KEY (tag, platform)
);

ALTER TABLE geode_versions
    ADD COLUMN resources_url TEXT,
    ADD COLUMN resources_hash TEXT;
