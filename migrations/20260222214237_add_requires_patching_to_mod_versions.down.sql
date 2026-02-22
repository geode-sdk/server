-- Add down migration script here

ALTER TABLE mod_versions
DROP requires_patching;
