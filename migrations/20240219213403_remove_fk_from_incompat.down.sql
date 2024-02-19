-- Add down migration script here

alter table incompatibilities add constraint incompatibilities_incompatibility_id_fkey foreign key (incompatibility_id) references mods(id);