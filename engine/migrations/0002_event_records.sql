CREATE TABLE IF NOT EXISTS event_records (
    id BIGSERIAL PRIMARY KEY,
    username TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    event_type TEXT NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    color TEXT NOT NULL,
    payload JSONB NOT NULL,
    is_read BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_event_records_user_time
    ON event_records(username, timestamp);

CREATE INDEX IF NOT EXISTS idx_event_records_user_unread
    ON event_records(username, is_read);
