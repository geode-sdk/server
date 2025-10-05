-- Add up migration script here

ALTER TABLE developers
ADD COLUMN superadmin BOOLEAN DEFAULT false NOT NULL;
