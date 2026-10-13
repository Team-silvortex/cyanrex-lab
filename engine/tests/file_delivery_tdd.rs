#![cfg(unix)]
//! Explicit loopback delivery to a trusted-host target, not mounted UI or current grant authority.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkClientError, FileChunkTransport,
            FileDownloadError, FileGrantedDownloadError, FileGrantedSaveError,
            FileManifestClientError,
        },
        file_chunk_receiver::FileChunkReceiveError,
        file_transfer::{
            staging::{ChunkStaging, ChunkStagingError, FileSaveError, FileSaveTarget},
            verify_chunk, verify_file, FILE_TRANSFER_BUFFER_BYTES,
        },
    },
};
use uuid::Uuid;

#[path = "file_delivery/admission.rs"]
mod admission;
#[path = "file_delivery/baseline.rs"]
mod baseline;
#[path = "file_delivery/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_delivery/cancellation.rs"]
mod cancellation;
#[path = "file_delivery/failures.rs"]
mod failures;
#[path = "file_delivery/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "file_download_client/fixture.rs"]
mod legacy_fixture;
#[allow(dead_code)]
#[path = "file_manifest_client/reply.rs"]
mod reply;
#[path = "file_delivery/source.rs"]
mod source;
#[path = "file_delivery/support.rs"]
mod support;
