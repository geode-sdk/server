-- Add up migration script here

CREATE TABLE IF NOT EXISTS developer_login_info (
    id BIGINT NOT NULL PRIMARY KEY,
    email TEXT NOT NULL,
    email_verified_at DATETIMETZ,
    password TEXT NOT NULL,
    FOREIGN KEY (id) REFERENCES developers(id) ON DELETE CASCADE
);

