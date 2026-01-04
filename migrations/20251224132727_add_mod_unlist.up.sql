-- Add up migration script here

ALTER TABLE mods
ADD unlisted BOOLEAN DEFAULT FALSE NOT NULL;

CREATE TABLE mod_unlist_history(
    id BIGINT GENERATED ALWAYS AS IDENTITY,
    mod_id TEXT NOT NULL,
    unlisted BOOLEAN NOT NULL,
    details TEXT,
    modified_by INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,

    FOREIGN KEY (mod_id) REFERENCES mods(id),
    FOREIGN KEY (modified_by) REFERENCES developers(id)
);

CREATE INDEX idx_mod_unlist_history_mod_id ON mod_unlist_history(mod_id);