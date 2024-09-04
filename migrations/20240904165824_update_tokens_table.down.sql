-- Add down migration script here

DELETE FROM auth_tokens;

DROP INDEX auth_tokens_token_idx;
DROP INDEX auth_tokens_refresh_token_idx;

ALTER TABLE auth_tokens DROP CONSTRAINT auth_tokens_pkey;
ALTER TABLE auth_tokens DROP id;
ALTER TABLE auth_tokens DROP token_expire;
ALTER TABLE auth_tokens DROP refresh_token;
ALTER TABLE auth_tokens DROP refresh_token_expire;
ALTER TABLE auth_tokens DROP created_at;
ALTER TABLE auth_tokens DROP updated_at;
ALTER TABLE auth_tokens ADD PRIMARY KEY(token);
