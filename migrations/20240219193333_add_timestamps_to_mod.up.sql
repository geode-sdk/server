-- Add up migration script here

alter table mods add column created_at timestamptz not null default now();
alter table mods add column updated_at timestamptz not null default now();

alter table mod_versions add column created_at timestamptz not null default now();
alter table mod_versions add column updated_at timestamptz not null default now();