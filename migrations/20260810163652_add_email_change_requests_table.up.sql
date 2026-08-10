-- Add up migration script here

CREATE TABLE email_change_requests (
    developer_id INTEGER NOT NULL PRIMARY KEY,
    token UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    new_email TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,

    FOREIGN KEY (developer_id) REFERENCES developers(id) ON DELETE CASCADE
);
