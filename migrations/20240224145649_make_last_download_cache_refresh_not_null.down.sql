-- Add down migration script here

alter table mods alter column last_download_cache_refresh drop not null;
alter table mods alter column last_download_cache_refresh drop default;
alter table mod_versions alter column last_download_cache_refresh drop not null;
alter table mod_versions alter column last_download_cache_refresh drop default;