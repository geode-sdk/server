-- Add up migration script here

CREATE TABLE deprecations (
    id SERIAL PRIMARY KEY NOT NULL,
    mod_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    updated_by INT,

    FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE CASCADE,
    FOREIGN KEY (updated_by) REFERENCES developers(id) ON DELETE SET NULL
);

CREATE INDEX idx_deprecations_mod_id ON deprecations(mod_id);

CREATE TABLE deprecated_by (
    id SERIAL PRIMARY KEY NOT NULL,
    deprecation_id INTEGER NOT NULL,
    by_mod_id TEXT NOT NULL,
    -- If we want to add mod-specific reasons, add them here
    FOREIGN KEY (deprecation_id) REFERENCES deprecations(id) ON DELETE CASCADE,
    FOREIGN KEY (by_mod_id) REFERENCES mods(id)
);

CREATE INDEX idx_deprecated_by_deprecation_id ON deprecated_by(deprecation_id);
