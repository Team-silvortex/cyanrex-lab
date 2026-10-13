#![cfg(unix)]
//! Explicit native loopback download composition, not a listener or current grant authority.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkClientError, FileChunkTransport,
            FileDownloadError,
        },
        file_chunk_receiver::FileChunkReceiveError,
        file_transfer::{
            staging::{ChunkStaging, ChunkStagingError},
            verify_chunk, verify_file, FILE_TRANSFER_BUFFER_BYTES,
        },
    },
};
use uuid::Uuid;

#[path = "file_download_client/admission.rs"]
mod admission;
#[path = "file_download_client/baseline.rs"]
mod baseline;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_download_client/cancellation.rs"]
mod cancellation;
#[path = "file_download_client/faults.rs"]
mod faults;
#[path = "file_download_client/fixture.rs"]
mod fixture;
#[path = "file_download_client/source.rs"]
mod source;
#[path = "file_download_client/support.rs"]
mod support;
