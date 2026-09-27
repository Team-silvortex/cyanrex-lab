-- Explicit C1-E empty-source installation only; never startup or an existing-account backfill.
CREATE TABLE collaboration_auth_source_schema (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    version INTEGER NOT NULL,
    authority_id UUID NOT NULL CHECK (authority_id <> '00000000-0000-0000-0000-000000000000')
);
-- cyanrex-statement
ALTER TABLE users ADD COLUMN account_id UUID NOT NULL
    CHECK (account_id <> '00000000-0000-0000-0000-000000000000');
-- cyanrex-statement
ALTER TABLE users ADD CONSTRAINT collaboration_auth_account_id_unique UNIQUE (account_id);
-- cyanrex-statement
ALTER TABLE users ADD CONSTRAINT collaboration_auth_account_coordinate UNIQUE (username, account_id);
-- cyanrex-statement
ALTER TABLE sessions ADD COLUMN account_id UUID NOT NULL;
-- cyanrex-statement
ALTER TABLE sessions ADD CONSTRAINT collaboration_auth_session_account
    FOREIGN KEY (username, account_id) REFERENCES users(username, account_id) ON DELETE CASCADE;
-- cyanrex-statement
ALTER TABLE sessions ADD CONSTRAINT collaboration_auth_session_digest
    CHECK (token COLLATE "C" ~ '^[0-9a-f]{64}$');
-- cyanrex-statement
CREATE FUNCTION collaboration_auth_reject_reassignment() RETURNS trigger
    LANGUAGE plpgsql AS $$ BEGIN
        IF NEW.username IS DISTINCT FROM OLD.username OR NEW.account_id IS DISTINCT FROM OLD.account_id THEN
            RAISE EXCEPTION 'auth account coordinates are immutable';
        END IF;
        IF TG_TABLE_NAME = 'sessions' AND to_jsonb(NEW)->>'token' IS DISTINCT FROM to_jsonb(OLD)->>'token' THEN
            RAISE EXCEPTION 'auth session token is immutable';
        END IF;
        RETURN NEW;
    END $$;
-- cyanrex-statement
CREATE TRIGGER collaboration_auth_account_immutable BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION collaboration_auth_reject_reassignment();
-- cyanrex-statement
CREATE TRIGGER collaboration_auth_session_immutable BEFORE UPDATE ON sessions
    FOR EACH ROW EXECUTE FUNCTION collaboration_auth_reject_reassignment();
