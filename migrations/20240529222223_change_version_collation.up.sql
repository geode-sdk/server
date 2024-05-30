-- Add up migration script here

CREATE COLLATION IF NOT EXISTS en_natural (
   LOCALE = 'en-US-u-kn-true',
   PROVIDER = 'icu'
);

ALTER TABLE incompatibilities
ALTER COLUMN version 
    SET DATA TYPE TEXT 
    COLLATE "en_natural";

ALTER TABLE dependencies 
ALTER COLUMN version 
    SET DATA TYPE TEXT 
    COLLATE "en_natural";

ALTER TABLE mod_versions
ALTER COLUMN version 
    SET DATA TYPE TEXT 
    COLLATE "en_natural";