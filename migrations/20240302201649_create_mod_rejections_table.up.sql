-- Add up migration script here

create table mod_rejections(
    id serial primary key,
    version text not null,
    reason text not null,
    created_at timestamptz not null default now(),
    mod_id text not null,
    admin_id integer not null,
    FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE CASCADE,
    FOREIGN KEY (admin_id) REFERENCES developers(id) ON DELETE CASCADE
);

create index mod_rejections_mod_id_version_idx on mod_rejections(mod_id, version);
create index mod_rejections_mod_id_idx on mod_rejections(mod_id);
create index mod_rejections_admin_id_idx on mod_rejections(admin_id);