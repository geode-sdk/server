-- Add down migration script here

drop index idx_mods_id_latest_version;
drop index idx_mod_versions_mod_id;
drop index idx_mod_versions_validated;
drop index idx_mod_tags_name;
drop index idx_mods_mod_tags_mod_id;
drop index idx_dependencies_dependent_id;
drop index idx_incompatibilities_mod_id;
drop index idx_developers_username;
drop index idc_mods_developers_mod_id;
drop index idx_auth_tokens_developer_id;


-- Fix for mod_downloads not having last_download_cache_refresh :(

alter table mod_downloads drop column if exists last_download_cache_refresh timestamptz;