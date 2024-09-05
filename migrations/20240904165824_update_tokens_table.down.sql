-- Add down migration script here

CREATE TABLE auth_tokens (
    token TEXT NOT NULL,
    developer_id INTEGER NOT NULL,
    PRIMARY KEY(token),
    FOREIGN KEY(developer_id) REFERENCES developers(id)
);

CREATE TABLE github_login_attempts (
    uid UUID DEFAULT gen_random_uuid() NOT NULL,
    ip inet NOT NULL,
    device_code TEXT NOT NULL,
    interval INTEGER NOT NULL,
    expires_in INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    last_poll TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    challenge_uri TEXT NOT NULL,
    user_code TEXT NOT NULL,
    PRIMARY KEY (uid, ip)
);

DROP TABLE oauth_attempts;