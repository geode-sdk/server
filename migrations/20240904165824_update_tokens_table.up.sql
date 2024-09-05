-- Add up migration script here

DROP TABLE auth_tokens;

CREATE TABLE oauth_attempts (
    uid UUID DEFAULT gen_random_uuid() NOT NULL,
    interval INTEGER NOT NULL,
    expires_in INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    last_poll TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    token TEXT,
    refresh_token TEXT,
    PRIMARY KEY (uid)
);

DROP TABLE github_login_attempts;