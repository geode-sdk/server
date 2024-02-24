-- Add up migration script here

create index idx_mods_id_latest_version on mods(id, latest_version);
create index idx_mod_versions_mod_id on mod_versions(mod_id);
create index idx_mod_versions_validated on mod_versions(validated);
create index idx_mod_tags_name on mod_tags(name);
create index idx_mods_mod_tags_mod_id on mods_mod_tags(mod_id);
create index idx_dependencies_dependent_id on dependencies(dependent_id);
create index idx_incompatibilities_mod_id on incompatibilities(mod_id);
create index idx_developers_username on developers(username);
create index idc_mods_developers_mod_id on mods_developers(mod_id);
create index idx_auth_tokens_developer_id on auth_tokens(developer_id);

-- Fix for mod_downloads not having last_download_cache_refresh :(

alter table mod_downloads add column if not exists last_download_cache_refresh timestamptz;