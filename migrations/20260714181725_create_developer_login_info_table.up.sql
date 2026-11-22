-- Add up migration script here

CREATE TABLE IF NOT EXISTS developer_login_info (
    developer_id INTEGER NOT NULL PRIMARY KEY,
    email TEXT NOT NULL,
    email_verified_at TIMESTAMPTZ,
    password TEXT NOT NULL,

    FOREIGN KEY (developer_id) REFERENCES developers(id) ON DELETE CASCADE
);

