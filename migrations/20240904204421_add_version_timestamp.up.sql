-- Add up migration script here

ALTER TABLE mod_versions ADD column created_at timestamptz DEFAULT now();
ALTER TABLE mod_versions ADD column updated_at timestamptz DEFAULT now();

-- set old versions to null, as we don't have their timestamps
UPDATE mod_versions SET created_at=NULL, updated_at=NULL;
