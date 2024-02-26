-- Add down migration script here

alter table mod_gd_versions 
drop constraint mod_gd_versions_mod_id_fkey,
add constraint mod_gd_versions_mod_id_fkey
	foreign key (mod_id)
	references mod_versions(id);

alter table mod_downloads
drop constraint mod_downloads_mod_version_id_fkey,
add constraint mod_downloads_mod_version_id_fkey
    foreign key (mod_version_id)
    references mod_versions(id);

alter table dependencies
drop constraint dependencies_dependent_id_fkey,
add constraint dependencies_dependent_id_fkey
    foreign key (dependent_id)
    references mod_versions(id);

alter table incompatibilities
drop constraint incompatibilities_mod_id_fkey,
add constraint incompatibilities_mod_id_fkey
    foreign key (mod_id)
    references mod_versions(id);

alter table mods_developers
drop constraint mods_developers_mod_id_fkey,
drop constraint mods_developers_developer_id_fkey,
add constraint mods_developers_mod_id_fkey
    foreign key (mod_id)
    references mods(id),
add constraint mods_developers_developer_id_fkey
    foreign key (developer_id)
    references developers(id);

alter table mods_mod_tags
drop constraint mods_mod_tags_mod_id_fkey,
drop constraint mods_mod_tags_tag_id_fkey,
add constraint mods_mod_tags_mod_id_fkey
    foreign key (mod_id)
    references mods(id),
add constraint mods_mod_tags_tag_id_fkey
    foreign key (tag_id)
    references mod_tags(id);

alter table mod_versions
drop constraint mod_versions_mod_id_fkey,
add constraint mod_versions_mod_id_fkey
    foreign key (mod_id)
    references mods(id);

