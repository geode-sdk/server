-- Add down migration script here

alter table github_login_attempts drop column if exists challenge_uri;
alter table github_login_attempts drop column if exists user_code;