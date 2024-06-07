-- Add up migration script here

UPDATE mod_gd_versions SET platform = 'mac-intel' WHERE platform = 'mac';

ALTER TYPE gd_ver_platform RENAME TO _gd_ver_platform;
CREATE TYPE gd_ver_platform AS enum('android32', 'android64', 'ios', 'mac-arm', 'mac-intel', 'win');
ALTER TABLE mod_gd_versions RENAME COLUMN platform TO _platform;

ALTER TABLE mod_gd_versions ADD COLUMN platform gd_ver_platform NOT NULL default 'win';
UPDATE mod_gd_versions SET platform = _platform::text::gd_ver_platform;
ALTER TABLE mod_gd_versions DROP COLUMN _platform;
DROP TYPE _gd_ver_platform;