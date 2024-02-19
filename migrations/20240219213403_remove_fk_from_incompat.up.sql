-- Add up migration script here

alter table incompatibilities drop constraint if exists incompatibilities_incompatibility_id_fkey;