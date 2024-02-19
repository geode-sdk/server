-- Add down migration script here

alter table mods drop column if exists created_at;
alter table mods drop column if exists updated_at;

alter table mod_versions drop column if exists created_at;
alter table mod_versions drop column if exists updated_at;