-- Add down migration script here

delete from auth_tokens;

alter table auth_tokens alter column token type uuid using token::uuid;
alter table auth_tokens alter column token set default gen_random_uuid();