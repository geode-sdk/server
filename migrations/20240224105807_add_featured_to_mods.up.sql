-- Add up migration script here

alter table mods add column featured boolean not null default false;