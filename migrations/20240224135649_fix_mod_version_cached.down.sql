-- Add down migration script here

alter table mod_downloads add column if not exists last_download_cache_refresh timestamptz;
alter table mod_versions drop column if exists last_download_cache_refresh;