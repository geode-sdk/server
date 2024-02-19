-- Add down migration script here

alter table dependencies add constraint dependencies_dependency_id_fkey foreign key (dependency_id) references mods(id);