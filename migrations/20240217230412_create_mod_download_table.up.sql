-- Add up migration script here

create table mod_downloads (
    mod_version_id integer not null,
    ip inet not null,
    time_downloaded timestamptz not null default now(),
    primary key (mod_version_id, ip),
    foreign key (mod_version_id) references mod_versions (id) on delete cascade
);

alter table mods add column download_count integer not null default 0;
alter table mods add column last_download_cache_refresh timestamptz;

alter table mod_versions add column download_count integer not null default 0;