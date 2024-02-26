-- Add up migration script here

alter table mods drop column latest_version;
drop index if exists idx_mods_id_latest_version;