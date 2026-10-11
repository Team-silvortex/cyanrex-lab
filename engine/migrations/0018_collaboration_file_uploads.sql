CREATE TABLE collaboration_file_upload_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL CHECK (version = 1),
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    installation_id UUID NOT NULL,
    namespace_oid BIGINT NOT NULL CHECK (namespace_oid > 0),
    schema_oid BIGINT NOT NULL CHECK (schema_oid > 0),
    uploads_oid BIGINT NOT NULL CHECK (uploads_oid > 0),
    total_bytes BIGINT NOT NULL CHECK (total_bytes BETWEEN 0 AND 17592186044416),
    owner_bytes BIGINT NOT NULL CHECK (owner_bytes BETWEEN 0 AND total_bytes),
    total_uploads INTEGER NOT NULL CHECK (total_uploads BETWEEN 1 AND 256),
    owner_uploads INTEGER NOT NULL CHECK (owner_uploads BETWEEN 1 AND total_uploads)
);
CREATE TABLE collaboration_file_uploads (
    upload_id UUID PRIMARY KEY,
    owner_id UUID NOT NULL,
    account_id UUID NOT NULL,
    manifest BYTEA NOT NULL CHECK (octet_length(manifest) <= 1179648)
);
