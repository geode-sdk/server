-- Add up migration script here
CREATE TYPE mod_importance AS ENUM ('required', 'recommended', 'suggested');

CREATE TABLE mods (
    id TEXT PRIMARY KEY NOT NULL,
    repository TEXT,
    latest_version TEXT NOT NULL,
    validated BOOLEAN NOT NULL
);

CREATE TABLE mod_versions (
    id SERIAL PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    version TEXT NOT NULL,
    download_link TEXT NOT NULL,
    hash TEXT NOT NULL,
    geode_version TEXT NOT NULL,
    windows BOOLEAN NOT NULL,
    android32 BOOLEAN NOT NULL,
    android64 BOOLEAN NOT NULL,
    mac BOOLEAN NOT NULL,
    ios BOOLEAN NOT NULL,
    early_load BOOLEAN NOT NULL,
    is_api_mod BOOLEAN NOT NULL,
    mod_id TEXT NOT NULL,
    FOREIGN KEY (mod_id) REFERENCES mods(id)
);

CREATE UNIQUE INDEX idx_version_id
ON mod_versions(version, mod_id);

CREATE TABLE mod_tags (
    id SERIAL PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE mods_mod_tags (
    mod_id INTEGER NOT NULL REFERENCES mod_versions(id),
    tag_id INTEGER NOT NULL REFERENCES mod_tags(id)
);

CREATE TABLE dependencies (
    dependent_id INTEGER NOT NULL,
    dependency_id INTEGER NOT NULL,
    compare TEXT NOT NULL,
    importance mod_importance NOT NULL,
    PRIMARY KEY (dependent_id, dependency_id),
    FOREIGN KEY (dependent_id) REFERENCES mod_versions(id),
    FOREIGN KEY (dependency_id) REFERENCES mod_versions(id)
);

CREATE TABLE incompatibilities (
    mod_id INTEGER NOT NULL,
    incompatibility_id INTEGER NOT NULL,
    compare TEXT NOT NULL,
    importance mod_importance NOT NULL,
    PRIMARY KEY (mod_id, incompatibility_id),
    FOREIGN KEY (mod_id) REFERENCES mod_versions(id)
);

CREATE TABLE developers (
    id SERIAL PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    display_name TEXT NOT NULL,
    verified BOOLEAN NOT NULL,
    github_user_id BIGINT NOT NULL
);

CREATE TABLE mods_developers (
    mod_id TEXT NOT NULL,
    developer_id INTEGER NOT NULL,
    PRIMARY KEY (mod_id, developer_id),
    FOREIGN KEY (mod_id) REFERENCES mods(id),
    FOREIGN KEY (developer_id) REFERENCES developers(id)
);
