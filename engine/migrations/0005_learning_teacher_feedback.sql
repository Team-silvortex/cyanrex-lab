ALTER TABLE learning_attempts
    ADD COLUMN IF NOT EXISTS teacher_feedback_reviewer TEXT,
    ADD COLUMN IF NOT EXISTS teacher_feedback_comment TEXT,
    ADD COLUMN IF NOT EXISTS teacher_feedback_revision BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS teacher_feedback_updated_at TIMESTAMPTZ;
