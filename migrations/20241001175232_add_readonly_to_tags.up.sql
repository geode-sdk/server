-- Add up migration script here

ALTER TABLE mod_tags
ADD readonly BOOLEAN NOT NULL DEFAULT false;