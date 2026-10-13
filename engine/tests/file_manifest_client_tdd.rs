#![cfg(unix)]
//! Explicit loopback metadata/chunk transport, not current grant authority or mounted delivery.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkClientError, FileChunkTransport,
            FileDownloadError, FileGrantedDownloadError, FileManifestClientError,
        },
        file_chunk_receiver::FileChunkReceiveError,
        file_transfer::{
            staging::{ChunkStaging, ChunkStagingError},
            verify_chunk, verify_file, FILE_TRANSFER_BUFFER_BYTES,
        },
    },
};
use uuid::Uuid;

#[path = "file_manifest_client/admission.rs"]
mod admission;
#[path = "file_manifest_client/baseline.rs"]
mod baseline;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_manifest_client/combination.rs"]
mod combination;
#[path = "file_manifest_client/contract.rs"]
mod contract;
#[path = "file_manifest_client/deadlines.rs"]
mod deadlines;
#[path = "file_manifest_client/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "file_download_client/fixture.rs"]
mod legacy_fixture;
#[path = "file_manifest_client/reply.rs"]
mod reply;
#[path = "file_manifest_client/source.rs"]
mod source;
#[path = "file_manifest_client/support.rs"]
mod support;
