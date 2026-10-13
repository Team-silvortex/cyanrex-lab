#![cfg(unix)]
//! Explicit native TLS acceptance; these opt-in cases do not authorize live route mounting.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkClientError, FileChunkTransport,
            FileControlledSaveError, FileDeliveryCancelHandle, FileDeliveryObserver,
            FileDeliveryPhase, FileDeliveryProgress, FileDeliverySnapshot, FileDownloadError,
            FileGrantedDownloadError, FileGrantedSaveError, FileManifestClientError,
        },
        file_chunk_receiver::FileChunkReceiveError,
        file_transfer::{
            staging::{ChunkStaging, ChunkStagingError, FileSaveError, FileSaveTarget},
            verify_chunk, verify_file, FILE_TRANSFER_BUFFER_BYTES,
        },
    },
};
use uuid::Uuid;

#[path = "file_delivery_tls/acceptance.rs"]
mod acceptance;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_delivery_tls/cancellation.rs"]
mod cancellation;
#[path = "file_delivery_tls/certificates.rs"]
mod certificates;
#[path = "file_delivery_tls/fixture.rs"]
mod fixture;
#[allow(dead_code, unused_imports)]
#[path = "file_download_client/fixture.rs"]
mod legacy_fixture;
#[path = "file_delivery_tls/probe.rs"]
mod probe;
#[allow(dead_code)]
#[path = "file_manifest_client/reply.rs"]
mod reply;
#[path = "file_delivery_tls/security.rs"]
mod security;
#[path = "file_delivery_tls/server.rs"]
mod server;
#[path = "file_delivery_tls/support.rs"]
mod support;
