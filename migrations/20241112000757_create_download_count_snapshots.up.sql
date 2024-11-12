-- Add up migration script here

CREATE TABLE IF NOT EXISTS mod_versions_download_count_snapshots(
    id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    mod_version_id INTEGER NOT NULL,
    download_count INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    FOREIGN KEY (mod_version_id) 
        REFERENCES mod_versions(id) 
        ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS mods_download_count_snapshots(
    id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    mod_id TEXT NOT NULL,
    download_count INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    FOREIGN KEY (mod_id) 
        REFERENCES mods(id) 
        ON DELETE CASCADE
)
