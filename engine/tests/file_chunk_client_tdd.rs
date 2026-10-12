#![cfg(unix)]
//! Explicit loopback transport fixtures, not a production listener or current authorization proof.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkClientError, FileChunkTransport,
        },
        file_transfer::{staging::ChunkStaging, verify_chunk},
    },
};
use uuid::Uuid;

#[path = "file_chunk_client/admission.rs"]
mod admission;
#[path = "file_chunk_client/baseline.rs"]
mod baseline;
#[path = "file_chunk_client/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_chunk_client/config.rs"]
mod config;
#[path = "file_chunk_client/deadlines.rs"]
mod deadlines;
#[path = "file_chunk_client/faults.rs"]
mod faults;
#[path = "file_chunk_client/fixture.rs"]
mod fixture;
#[path = "file_chunk_client/source.rs"]
mod source;
#[path = "file_chunk_client/support.rs"]
mod support;
#[path = "file_chunk_client/tls.rs"]
mod tls;
#[path = "file_chunk_client/tls_fixture.rs"]
mod tls_fixture;
