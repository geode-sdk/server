CREATE INDEX idx_mod_version_statuses_status_version ON mod_version_statuses (status, mod_version_id);
CREATE INDEX idx_mod_versions_mod_id_id_desc ON mod_versions (mod_id, id DESC);

CREATE INDEX idx_mv_alpha_optimization ON mod_versions (id, mod_id)
WHERE (geode_meta IS NULL OR geode_meta NOT ILIKE 'alpha%');
CREATE INDEX idx_mgv_platform_gd_mod
ON mod_gd_versions (platform, gd, mod_id);
