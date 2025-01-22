-- Add up migration script here

ALTER TABLE auth_tokens
ADD COLUMN expires_at TIMESTAMPTZ NULL;

CREATE TABLE refresh_tokens (
    token TEXT NOT NULL,
    developer_id INTEGER NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY(token),
    FOREIGN KEY(developer_id) REFERENCES developers(id)
);

CREATE TABLE github_web_logins (
    state UUID NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);