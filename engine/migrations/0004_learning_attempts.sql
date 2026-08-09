CREATE TABLE IF NOT EXISTS learning_attempts (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    lab_id TEXT NOT NULL,
    template_id TEXT,
    source TEXT NOT NULL,
    source_sha256 TEXT NOT NULL,
    run_success BOOLEAN NOT NULL,
    stage TEXT NOT NULL,
    attach_expected BOOLEAN NOT NULL,
    attach_verified BOOLEAN NOT NULL,
    completed BOOLEAN NOT NULL,
    feedback TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_learning_attempts_user_lab_created
    ON learning_attempts(username, lab_id, created_at DESC);
