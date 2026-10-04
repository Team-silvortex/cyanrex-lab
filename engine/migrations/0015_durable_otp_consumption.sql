-- Fresh explicit source schema 2 installation only, never an existing-account migration.
-- The account incarnation owns this high watermark. Password changes must never reset it.
ALTER TABLE users ADD COLUMN otp_last_counter BIGINT NOT NULL DEFAULT '-1'::bigint
    CONSTRAINT collaboration_auth_otp_last_counter CHECK (otp_last_counter >= '-1'::bigint);
