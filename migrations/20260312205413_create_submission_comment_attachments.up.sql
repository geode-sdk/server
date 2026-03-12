-- Add up migration script here
CREATE TABLE mod_version_submission_comment_attachments
(
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    comment_id BIGINT      NOT NULL,
    filename   TEXT        NOT NULL, -- SHA-256 hex of WebP content; file on disk: {STORAGE_PATH}/submission_attachments/{filename}
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (comment_id) REFERENCES mod_version_submission_comments (id) ON DELETE CASCADE
);

CREATE INDEX idx_submission_attachments_comment_id
    ON mod_version_submission_comment_attachments (comment_id);

-- For fast COUNT lookups on delete (deduplication check)
CREATE INDEX idx_submission_attachments_filename
    ON mod_version_submission_comment_attachments (filename);
