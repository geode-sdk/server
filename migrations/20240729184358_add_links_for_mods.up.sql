-- Add up migration script here

CREATE TABLE mod_links (
    mod_id TEXT PRIMARY KEY NOT NULL,
    community TEXT,
    homepage TEXT,
    source TEXT,

    FOREIGN KEY (mod_id) REFERENCES mods(id)
);