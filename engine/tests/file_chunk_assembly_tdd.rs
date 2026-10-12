#![cfg(unix)]
//! Private local assembly fixtures, not publication, current authority or whole-file user saving.
use cyanrex_engine::{
    models::collaboration::*,
    services::file_transfer::{
        staging::{ChunkStaging, ChunkStagingError},
        verify_file, FILE_TRANSFER_BUFFER_BYTES,
    },
};
use uuid::Uuid;

#[path = "file_chunk_assembly/baseline.rs"]
mod baseline;
#[path = "file_chunk_assembly/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_chunk_assembly/faults.rs"]
mod faults;
#[path = "file_chunk_assembly/fixture.rs"]
mod fixture;
#[path = "file_chunk_assembly/roots.rs"]
mod roots;
