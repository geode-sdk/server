-- Add up migration script here

create table if not exists geode_versions (
    id serial primary key not null,
    version text not null,
    changelog text not null,
    win text not null,
    mac text not null,
    android32 text not null,
    android64 text not null,
    ios text not null,
    resources text not null
);