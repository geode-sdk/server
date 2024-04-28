-- Add up migration script here

create type mod_version_status as enum('pending', 'rejected', 'accepted', 'unlisted');

create table mod_version_statuses(
    id serial primary key,
    status mod_version_status not null default 'pending',
    info text,
    updated_at timestamptz not null default now(),
    mod_version_id integer not null,
    admin_id integer,
    foreign key (mod_version_id) 
        references mod_versions(id) 
        on delete cascade,
    foreign key (admin_id)
        references developers(id)
        on delete set null
);

create index mod_version_statuses_admin_id_idx on mod_version_statuses(admin_id);
create index mod_version_statuses_mod_version_id_idx on mod_version_statuses(mod_version_id);
create index mod_version_statuses_updated_at_idx on mod_version_statuses(updated_at);

alter table mod_versions add column status_id integer;

insert into mod_version_statuses (status, mod_version_id)
    select cast(case
        when validated = true then 'accepted'
        else 'pending'
    end AS mod_version_status) as status,
    id as mod_version_id
    from mod_versions;

update mod_versions set status_id = mvs.id
    from mod_version_statuses mvs
    where mod_versions.id = mvs.mod_version_id;

alter table mod_versions alter column status_id set not null;
alter table mod_versions 
    add foreign key (status_id) 
    references mod_version_statuses(id)
    deferrable;
alter table mod_versions drop column validated;

create index mod_versions_status_id_idx on mod_versions(status_id);