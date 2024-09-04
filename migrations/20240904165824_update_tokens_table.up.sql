-- Add up migration script here

DELETE FROM auth_tokens;

ALTER TABLE auth_tokens DROP CONSTRAINT auth_tokens_pkey;
ALTER TABLE auth_tokens ADD id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY;
ALTER TABLE auth_tokens ADD token_expire TIMESTAMPTZ NOT NULL;
ALTER TABLE auth_tokens ADD refresh_token TEXT NOT NULL;
ALTER TABLE auth_tokens ADD refresh_token_expire TIMESTAMPTZ NOT NULL;
ALTER TABLE auth_tokens ADD created_at TIMESTAMPTZ NOT NULL;
ALTER TABLE auth_tokens ADD updated_at TIMESTAMPTZ NOT NULL;

CREATE INDEX auth_tokens_token_idx ON auth_tokens(token);
CREATE INDEX auth_tokens_refresh_token_idx ON auth_tokens(refresh_token);
