-- Add down migration script here

-- dependencies
ALTER TABLE dependencies
DROP COLUMN windows;

ALTER TABLE dependencies
DROP COLUMN mac_arm;

ALTER TABLE dependencies
DROP COLUMN mac_intel;

ALTER TABLE dependencies
DROP COLUMN android32;

ALTER TABLE dependencies
DROP COLUMN android64;

ALTER TABLE dependencies
DROP COLUMN ios;

-- incompatibilities
ALTER TABLE incompatibilities
DROP COLUMN windows;

ALTER TABLE incompatibilities
DROP COLUMN mac_arm;

ALTER TABLE incompatibilities
DROP COLUMN mac_intel;

ALTER TABLE incompatibilities
DROP COLUMN android32;

ALTER TABLE incompatibilities
DROP COLUMN android64;

ALTER TABLE incompatibilities
DROP COLUMN ios;
