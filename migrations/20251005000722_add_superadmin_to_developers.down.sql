-- Add down migration script here

ALTER TABLE developers
DROP COLUMN superadmin;
