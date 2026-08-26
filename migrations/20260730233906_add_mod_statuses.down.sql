-- Add down migration script here

drop index mods_status_idx;

alter table mods
	drop column status,
	drop column status_info,
	drop column status_locked;

drop table mod_status_logs;
drop type mod_status;