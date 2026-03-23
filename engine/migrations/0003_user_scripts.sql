CREATE TABLE IF NOT EXISTS user_scripts (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    title TEXT NOT NULL,
    script TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_user_scripts_username_updated
    ON user_scripts(username, updated_at DESC);
