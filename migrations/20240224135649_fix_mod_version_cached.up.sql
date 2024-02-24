-- Add up migration script here

alter table mod_downloads drop column if exists last_download_cache_refresh timestamptz;
alter table mod_versions add column if not exists last_download_cache_refresh timestamptz;