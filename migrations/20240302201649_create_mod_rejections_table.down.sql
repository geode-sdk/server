-- Add down migration script here

drop index mod_versions_status_id_idx;

alter table mod_versions add column validated boolean not null default false;
update mod_versions set validated = true
    from mod_version_statuses mvs
    where mod_versions.id = mvs.mod_version_id
    and mvs.status = 'accepted';

alter table mod_versions drop column status_id;
drop table mod_version_statuses;
drop type mod_version_status;