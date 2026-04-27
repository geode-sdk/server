-- Add up migration script here
CREATE TABLE bans (
	developer_id INTEGER PRIMARY KEY NOT NULL REFERENCES developers(id) ON DELETE CASCADE,
	reason TEXT,
	admin_id INTEGER REFERENCES developers(id) ON DELETE SET NULL,
	created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);
