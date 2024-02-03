-- Add up migration script here
CREATE TYPE dependency_importance AS ENUM ('required', 'recommended', 'suggested');
CREATE TYPE incompatibility_importance AS ENUM ('breaking', 'conflicting');
CREATE TYPE version_compare AS ENUM ('=', '>', '<', '>=', '=<');
CREATE TYPE gd_version as ENUM ('*', '2.113', '2.200', '2.204', '2.205');
CREATE TYPE gd_ver_platform as ENUM ('android32', 'android64', 'ios', 'mac', 'win');

CREATE TABLE mods (
    id TEXT PRIMARY KEY NOT NULL,
    repository TEXT,
    changelog TEXT,
    about TEXT,
    latest_version TEXT NOT NULL
);

CREATE TABLE mod_versions (
    id SERIAL PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    version TEXT NOT NULL,
    download_link TEXT NOT NULL,
    hash TEXT NOT NULL,
    geode TEXT NOT NULL,
    early_load BOOLEAN NOT NULL DEFAULT false,
    api BOOLEAN NOT NULL DEFAULT false,
    validated BOOLEAN NOT NULL,
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
    mod_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (mod_id, tag_id),
    FOREIGN KEY (mod_id) REFERENCES mod_versions(id),
    FOREIGN KEY (tag_id) REFERENCES mod_tags(id)
);

CREATE TABLE mod_gd_versions (
    id SERIAL PRIMARY KEY NOT NULL,
    mod_id INTEGER NOT NULL,
    gd gd_version NOT NULL,
    platform gd_ver_platform NOT NULL,
    FOREIGN KEY (mod_id) REFERENCES mod_versions(id)
);

CREATE TABLE dependencies (
    dependent_id INTEGER NOT NULL,
    dependency_id INTEGER NOT NULL,
    compare version_compare NOT NULL,
    importance dependency_importance NOT NULL,
    PRIMARY KEY (dependent_id, dependency_id),
    FOREIGN KEY (dependent_id) REFERENCES mod_versions(id),
    FOREIGN KEY (dependency_id) REFERENCES mod_versions(id)
);

CREATE TABLE incompatibilities (
    mod_id INTEGER NOT NULL,
    incompatibility_id INTEGER NOT NULL,
    compare version_compare NOT NULL,
    importance incompatibility_importance NOT NULL,
    PRIMARY KEY (mod_id, incompatibility_id),
    FOREIGN KEY (mod_id) REFERENCES mod_versions(id),
    FOREIGN KEY (incompatibility_id) REFERENCES mod_versions(id)
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

CREATE TABLE github_login_attempts (
    uid UUID DEFAULT gen_random_uuid() NOT NULL,
    ip inet NOT NULL,
    device_code TEXT NOT NULL,
    interval INTEGER NOT NULL,
    expires_in INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    last_poll TIMESTAMPTZ,
    PRIMARY KEY (uid, ip)
);