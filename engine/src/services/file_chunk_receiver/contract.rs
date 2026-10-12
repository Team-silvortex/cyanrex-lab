use super::{FileChunkReceiveError, FileTransferManifest};
use crate::models::collaboration::FileChunkSpec;
use axum::http::{HeaderMap, Response, StatusCode};

const MAX_HEADERS: usize = 64;
const MAX_HEADER_BYTES: usize = 8192;

pub(super) fn validate<B>(
    response: &Response<B>,
    manifest: &FileTransferManifest,
    spec: &FileChunkSpec,
) -> Result<(), FileChunkReceiveError> {
    let invalid = FileChunkReceiveError::InvalidResponse;
    let headers = response.headers();
    if response.status() != StatusCode::OK || headers.len() > MAX_HEADERS {
        return Err(invalid);
    }
    let mut size = 0usize;
    for (name, value) in headers {
        // Repeated values count independently, including their repeated field-name bytes.
        size = size
            .saturating_add(name.as_str().len())
            .saturating_add(value.as_bytes().len());
        if size > MAX_HEADER_BYTES {
            return Err(invalid);
        }
    }
    for name in [
        "content-encoding",
        "content-range",
        "transfer-encoding",
        "trailer",
        "set-cookie",
        "location",
        "etag",
        "last-modified",
        "retry-after",
    ] {
        if headers.contains_key(name) {
            return Err(invalid);
        }
    }
    for (name, expected) in [
        ("content-type", "application/octet-stream"),
        ("content-disposition", "attachment; filename=\"chunk.bin\""),
        ("x-cyanrex-file-transfer-version", "1"),
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
    ] {
        require(headers, name, expected)?;
    }
    for (name, expected) in [
        ("content-length", spec.byte_length.to_string()),
        ("x-cyanrex-file-id", manifest.file_id().to_string()),
        (
            "x-cyanrex-file-version-id",
            manifest.version_id().to_string(),
        ),
        ("x-cyanrex-chunk-index", spec.index.to_string()),
        ("x-cyanrex-chunk-sha256", spec.sha256.to_string()),
        ("x-cyanrex-file-sha256", manifest.sha256().to_string()),
    ] {
        require(headers, name, &expected)?;
    }
    // Other bounded headers convey no path, ownership, grants or private server storage keys.
    Ok(())
}

fn require(headers: &HeaderMap, name: &str, expected: &str) -> Result<(), FileChunkReceiveError> {
    let mut values = headers.get_all(name).iter();
    if values.next().map(|value| value.as_bytes()) != Some(expected.as_bytes())
        || values.next().is_some()
    {
        return Err(FileChunkReceiveError::InvalidResponse);
    }
    Ok(())
}
