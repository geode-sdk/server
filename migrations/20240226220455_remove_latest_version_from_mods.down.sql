-- Add down migration script here

alter table mods add column latest_version TEXT;
create index idx_mods_id_latest_version on mods(id, latest_version);