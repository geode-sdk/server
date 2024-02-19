-- Add down migration script here

ALTER TYPE version_compare RENAME TO version_compare_old;
CREATE TYPE version_compare AS ENUM ('=', '>', '<', '>=', '=<');
ALTER TABLE incompatibilities ALTER COLUMN compare TYPE version_compare USING compare::text::version_compare;
ALTER TABLE dependencies ALTER COLUMN compare TYPE version_compare USING compare::text::version_compare;

DROP TYPE version_compare_old;