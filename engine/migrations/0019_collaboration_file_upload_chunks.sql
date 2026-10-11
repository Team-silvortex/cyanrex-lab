CREATE TABLE collaboration_file_upload_chunk_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL CHECK (version = 1),
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    installation_id UUID NOT NULL,
    namespace_oid BIGINT NOT NULL CHECK (namespace_oid > 0),
    schema_oid BIGINT NOT NULL CHECK (schema_oid > 0),
    chunks_oid BIGINT NOT NULL CHECK (chunks_oid > 0),
    max_entries INTEGER NOT NULL CHECK (max_entries BETWEEN 1 AND 16384)
);
CREATE TABLE collaboration_file_upload_chunks (
    upload_id UUID NOT NULL,
    chunk_index INTEGER NOT NULL CHECK (chunk_index BETWEEN 0 AND 16383),
    owner_id UUID NOT NULL,
    account_id UUID NOT NULL,
    source_key BYTEA NOT NULL CHECK (octet_length(source_key) = 32),
    root_key BYTEA NOT NULL CHECK (octet_length(root_key) = 32),
    total_length BIGINT NOT NULL CHECK (total_length BETWEEN 1 AND 68719476736),
    chunk_length BIGINT NOT NULL CHECK (chunk_length BETWEEN 1 AND 4194304),
    sha256 TEXT NOT NULL CHECK (octet_length(sha256) = 64 AND sha256 ~ '^[0-9a-f]{64}$'),
    nonce UUID NOT NULL UNIQUE CHECK (
        substring(nonce::text FROM 15 FOR 1) = '4' AND
        substring(nonce::text FROM 20 FOR 1) IN ('8', '9', 'a', 'b')
    ),
    confirmed BOOLEAN NOT NULL,
    PRIMARY KEY (upload_id, chunk_index),
    CHECK (chunk_index::bigint * 4194304 < total_length),
    CHECK (chunk_length = LEAST(4194304, total_length - chunk_index::bigint * 4194304))
);
