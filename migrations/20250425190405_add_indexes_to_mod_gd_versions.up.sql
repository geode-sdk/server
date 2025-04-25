-- Add up migration script here
CREATE INDEX idx_mod_gd_versions_mod_id ON mod_gd_versions(mod_id);
CREATE INDEX idx_mod_gd_versions_gd ON mod_gd_versions(gd);
CREATE INDEX idx_mod_gd_versions_platform ON mod_gd_versions(platform);