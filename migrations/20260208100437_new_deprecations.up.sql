-- Add up migration script here

CREATE TABLE deprecations (
    id SERIAL PRIMARY KEY NOT NULL,
    mod_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE CASCADE
);
CREATE TABLE deprecated_by (
    deprecation_id INTEGER NOT NULL,
    by_mod_id TEXT NOT NULL,
    -- If we want to add mod-specific reasons, add them here
    FOREIGN KEY (deprecation_id) REFERENCES deprecations(id) ON DELETE CASCADE,
    FOREIGN KEY (by_mod_id) REFERENCES mods(id)
);
