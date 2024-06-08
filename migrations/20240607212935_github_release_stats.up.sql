-- Add up migration script here

CREATE TABLE github_loader_release_stats (
    id SERIAL PRIMARY KEY NOT NULL,
    total_download_count BIGINT NOT NULL,
    latest_loader_version TEXT NOT NULL,
    checked_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);
