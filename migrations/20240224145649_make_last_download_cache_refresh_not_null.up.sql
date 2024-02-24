-- Add up migration script here

update mods set last_download_cache_refresh = now() where last_download_cache_refresh is null;
update mod_versions set last_download_cache_refresh = now() where last_download_cache_refresh is null;

alter table mods alter column last_download_cache_refresh set default now();
alter table mods alter column last_download_cache_refresh set not null;
alter table mod_versions alter column last_download_cache_refresh set default now();
alter table mod_versions alter column last_download_cache_refresh set not null;