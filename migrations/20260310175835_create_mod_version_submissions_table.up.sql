-- Add up migration script here

CREATE TYPE audit_action AS ENUM (
    'created',
    'updated',
    'deleted',
    'restored'
);

CREATE TABLE mod_version_submissions (
    mod_version_id INT NOT NULL PRIMARY KEY,
    locked BOOLEAN NOT NULL DEFAULT FALSE,
    locked_by INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (mod_version_id) REFERENCES mod_versions(id) ON DELETE CASCADE,
    FOREIGN KEY (locked_by) REFERENCES developers(id) ON DELETE SET NULL
);

CREATE TABLE mod_version_submissions_audit (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    submission_id INT NOT NULL,
    action audit_action NOT NULL,
    details TEXT,
    performed_by INT,
    performed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (submission_id) REFERENCES mod_version_submissions(mod_version_id) ON DELETE CASCADE,
    FOREIGN KEY (performed_by) REFERENCES developers(id) ON DELETE SET NULL
);

CREATE INDEX idx_submissions_audit_submission_id ON mod_version_submissions_audit(submission_id);

CREATE TABLE mod_version_submission_comments (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    submission_id INT NOT NULL,
    author_id INT NOT NULL,
    comment TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    FOREIGN KEY (submission_id) REFERENCES mod_version_submissions(mod_version_id) ON DELETE CASCADE,
    FOREIGN KEY (author_id) REFERENCES developers(id) ON DELETE RESTRICT
);

CREATE INDEX idx_submission_comments_submission_id
    ON mod_version_submission_comments(submission_id);

CREATE TABLE mod_version_submission_comment_audit (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    comment_id BIGINT NOT NULL,
    action audit_action NOT NULL,
    details TEXT,
    performed_by INT,
    performed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (comment_id) REFERENCES mod_version_submission_comments(id) ON DELETE CASCADE,
    FOREIGN KEY (performed_by) REFERENCES developers(id) ON DELETE SET NULL
);
