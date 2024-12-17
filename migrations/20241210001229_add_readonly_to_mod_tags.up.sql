-- Add up migration script here

ALTER TABLE mod_tags
ADD is_readonly BOOLEAN NOT NULL DEFAULT false;