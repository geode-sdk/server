-- Add up migration script here

alter table github_login_attempts add column challenge_uri text not null;
alter table github_login_attempts add column user_code text not null;