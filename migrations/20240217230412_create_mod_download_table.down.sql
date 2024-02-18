-- Add down migration script here

drop table mod_downloads;

alter table mods drop column if exists download_count;
alter table mods drop column if exists last_download_cache_refresh;

alter table mod_versions drop column if exists download_count;