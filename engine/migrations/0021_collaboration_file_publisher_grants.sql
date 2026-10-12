CREATE TABLE collaboration_file_publisher_grant_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL CHECK (version = 1),
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    installation_id UUID NOT NULL,
    namespace_oid BIGINT NOT NULL CHECK (namespace_oid > 0),
    schema_oid BIGINT NOT NULL CHECK (schema_oid > 0),
    grants_oid BIGINT NOT NULL CHECK (grants_oid > 0),
    changes_oid BIGINT NOT NULL CHECK (changes_oid > 0),
    max_grants INTEGER NOT NULL CHECK (max_grants BETWEEN 1 AND 256)
);
CREATE TABLE collaboration_file_publisher_grants (
    grant_id UUID PRIMARY KEY,
    subject_id UUID NOT NULL,
    subject_account_id UUID NOT NULL,
    issuer_id UUID NOT NULL,
    issuer_account_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 2),
    not_before BIGINT NOT NULL CHECK (not_before >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > not_before AND expires_at <= 253402300799),
    grant_payload BYTEA NOT NULL CHECK (octet_length(grant_payload) <= 4096)
);
CREATE TABLE collaboration_file_publisher_grant_changes (
    grant_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 2),
    actor_id UUID NOT NULL,
    actor_account_id UUID NOT NULL,
    not_before BIGINT NOT NULL CHECK (not_before >= 0),
    expires_at BIGINT NOT NULL CHECK (expires_at > not_before AND expires_at <= 253402300799),
    recorded_at BIGINT NOT NULL CHECK (recorded_at BETWEEN 0 AND 253402300799),
    grant_payload BYTEA NOT NULL CHECK (octet_length(grant_payload) <= 4096),
    PRIMARY KEY (grant_id, revision)
);
