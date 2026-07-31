-- Add up migration script here

create type mod_status as enum('default', 'archived', 'unlisted');

create table mod_statuses(
    id serial primary key,
    status mod_status not null default 'default',
    info text,
    updated_at timestamptz not null default now(),
    mod_id TEXT not null,
    actor_id integer,
    locked BOOLEAN not null DEFAULT FALSE,
    foreign key (mod_id)
        references mods(id)
        on delete cascade,
    foreign key (actor_id)
        references developers(id)
        on delete set null
);

create index mod_statuses_actor_id_idx on mod_statuses(actor_id);
create unique index mod_statuses_mod_id_idx on mod_statuses(mod_id);

alter table mods add column status_id integer;

insert into mod_statuses (mod_id) SELECT id as mod_id from mods;

update mods set status_id = ms.id
    from mod_statuses ms
    where mods.id = ms.mod_id;

alter table mods alter column status_id set not null;
alter table mods
    add foreign key (status_id)
    references mod_statuses(id)
    deferrable;

create index mods_status_id_idx on mods(status_id);