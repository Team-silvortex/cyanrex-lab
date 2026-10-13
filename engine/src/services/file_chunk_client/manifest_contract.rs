use super::FileManifestClientError;
use crate::models::collaboration::MAX_FILE_TRANSFER_MANIFEST_BYTES;
use axum::http::{HeaderMap, Response, StatusCode};

const MAX_HEADERS: usize = 64;
const MAX_HEADER_BYTES: usize = 8192;

pub(super) fn validate<B>(response: &Response<B>) -> Result<usize, FileManifestClientError> {
    let invalid = FileManifestClientError::InvalidResponse;
    let headers = response.headers();
    if response.status() != StatusCode::OK || headers.len() > MAX_HEADERS {
        return Err(invalid);
    }
    let mut size = 0usize;
    for (name, value) in headers {
        size = size
            .saturating_add(name.as_str().len())
            .saturating_add(value.as_bytes().len());
        if size > MAX_HEADER_BYTES || name.as_str().starts_with("access-control-") {
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
        "content-disposition",
        "x-cyanrex-file-transfer-version",
        "x-cyanrex-file-id",
        "x-cyanrex-file-version-id",
        "x-cyanrex-chunk-index",
        "x-cyanrex-chunk-sha256",
        "x-cyanrex-file-sha256",
    ] {
        if headers.contains_key(name) {
            return Err(invalid);
        }
    }
    for (name, value) in [
        ("content-type", "application/json"),
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
    ] {
        if single(headers, name)? != value.as_bytes() {
            return Err(invalid);
        }
    }
    let raw = single(headers, "content-length")?;
    if raw.is_empty() || raw[0] == b'0' || !raw.iter().all(u8::is_ascii_digit) {
        return Err(invalid);
    }
    let length = std::str::from_utf8(raw)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|length| *length <= MAX_FILE_TRANSFER_MANIFEST_BYTES)
        .ok_or(invalid)?;
    Ok(length)
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a [u8], FileManifestClientError> {
    let mut values = headers.get_all(name).iter();
    let first = values
        .next()
        .ok_or(FileManifestClientError::InvalidResponse)?;
    if values.next().is_some() {
        return Err(FileManifestClientError::InvalidResponse);
    }
    Ok(first.as_bytes())
}

#[cfg(test)]
#[path = "manifest_contract_tests.rs"]
mod tests;
