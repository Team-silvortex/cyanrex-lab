#![cfg(unix)]
//! Explicit current-call cooperative cancellation, not job abort, cleanup or resumed delivery.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkClientError, FileChunkTransport,
            FileControlledSaveError, FileDeliveryCancelHandle, FileDeliveryCancellation,
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

#[path = "file_delivery_cancel/baseline.rs"]
mod baseline;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_delivery_cancel/cancellation.rs"]
mod cancellation;
#[path = "file_delivery_cancel/empty.rs"]
mod empty;
#[path = "file_delivery_cancel/errors.rs"]
mod errors;
#[path = "file_delivery_cancel/fixture.rs"]
mod fixture;
#[path = "file_delivery_cancel/handles.rs"]
mod handles;
#[path = "file_delivery_cancel/independence.rs"]
mod independence;
#[allow(dead_code)]
#[path = "file_download_client/fixture.rs"]
mod legacy_fixture;
#[path = "file_delivery_cancel/preflight.rs"]
mod preflight;
#[path = "file_delivery_cancel/readers.rs"]
mod readers;
#[allow(dead_code)]
#[path = "file_manifest_client/reply.rs"]
mod reply;
#[path = "file_delivery_cancel/support.rs"]
mod support;
