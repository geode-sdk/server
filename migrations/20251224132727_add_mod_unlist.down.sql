-- Add down migration script here

ALTER TABLE mods
DROP IF EXISTS unlisted;

DROP TABLE IF EXISTS mod_unlist_history;