-- Add down migration script here

drop index mods_status_id_idx;

alter table mods drop column status_id;
drop table mod_statuses;
drop type mod_status;