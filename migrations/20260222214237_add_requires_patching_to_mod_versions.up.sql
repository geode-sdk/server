-- Add up migration script here

ALTER TABLE mod_versions
ADD requires_patching BOOLEAN NOT NULL DEFAULT FALSE;
