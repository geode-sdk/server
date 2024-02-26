-- Add up migration script here

delete from auth_tokens;

alter table auth_tokens alter column token drop default;
alter table auth_tokens alter column token type text using token::text;