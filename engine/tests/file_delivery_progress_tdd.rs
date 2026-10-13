#![cfg(unix)]
//! Current-call host observation only, not UI wiring, remote authority or saved restart progress.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkClientError, FileChunkTransport,
            FileDeliveryObserver, FileDeliveryPhase, FileDeliveryProgress, FileDeliverySnapshot,
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

#[path = "file_delivery_progress/baseline.rs"]
mod baseline;
#[path = "file_delivery_progress/boundaries.rs"]
mod boundaries;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_delivery_progress/empty.rs"]
mod empty;
#[path = "file_delivery_progress/failures.rs"]
mod failures;
#[path = "file_delivery_progress/fixture.rs"]
mod fixture;
#[path = "file_delivery_progress/independence.rs"]
mod independence;
#[allow(dead_code)]
#[path = "file_download_client/fixture.rs"]
mod legacy_fixture;
#[path = "file_delivery_progress/preflight.rs"]
mod preflight;
#[path = "file_delivery_progress/readers.rs"]
mod readers;
#[allow(dead_code)]
#[path = "file_manifest_client/reply.rs"]
mod reply;
#[path = "file_delivery_progress/support.rs"]
mod support;
