-- Add up migration script here

create type mod_status as enum('default', 'archived', 'unlisted');

create table mod_status_logs(
    id serial primary key,
    mod_id TEXT not null,
    performed_at timestamptz not null default now(),
    actor_id integer,
    status mod_status not null default 'default',
    info text,
    locked BOOLEAN,
    foreign key (mod_id)
        references mods(id)
        on delete cascade,
    foreign key (actor_id)
        references developers(id)
        on delete set null
);

create index mod_statuses_actor_id_idx on mod_status_logs(actor_id);
create index mod_statuses_mod_id_idx on mod_status_logs(mod_id);

alter table mods
    add column status mod_status not null default 'default',
    add column status_info text,
    add column status_locked boolean not null default false;

create index mods_status_idx on mods(status);