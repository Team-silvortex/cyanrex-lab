use crate::{
    models::collaboration::{
        FileGrantId, FileId, FileUploadId, FileVersionId, FileVersionRef, Sha256Digest,
        WorkspaceRef, MAX_FILE_TRANSFER_BYTES, MAX_FILE_TRANSFER_CHUNKS,
    },
    platform_http::{errors::HttpError, file_selectors},
    services::auth_service::durable_source::{
        SessionFileChunkRead, SessionFileChunkReadRequest, SessionFileDownloadRequest,
    },
};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};

/// Raw canonical selectors only. Scope and owner attribution never come from the request.
pub(super) fn chunk_request(
    query: Option<&str>,
    scope: WorkspaceRef,
) -> Result<SessionFileChunkReadRequest, HttpError> {
    let [grant_id, upload_id, file_id, version_id, byte_length, sha256, chunk_index] =
        file_selectors::parse(
            query,
            [
                "grant_id",
                "upload_id",
                "file_id",
                "version_id",
                "byte_length",
                "sha256",
                "chunk_index",
            ],
        )?;
    let grant_id: FileGrantId = grant_id.parse().map_err(|_| HttpError::invalid_request())?;
    let upload_id: FileUploadId = upload_id
        .parse()
        .map_err(|_| HttpError::invalid_request())?;
    let file_id: FileId = file_id.parse().map_err(|_| HttpError::invalid_request())?;
    let version_id: FileVersionId = version_id
        .parse()
        .map_err(|_| HttpError::invalid_request())?;
    let byte_length = file_selectors::decimal(byte_length)?;
    let sha256: Sha256Digest = sha256.parse().map_err(|_| HttpError::invalid_request())?;
    let chunk_index = file_selectors::decimal(chunk_index)?;
    if byte_length > MAX_FILE_TRANSFER_BYTES || chunk_index >= MAX_FILE_TRANSFER_CHUNKS as u64 {
        return Err(HttpError::invalid_request());
    }
    // This small host-built metadata object follows strict raw duplicate and lexical checks;
    // it is not a client JSON Value and cannot erase untrusted duplicate fields or choose scope.
    let expected_file: FileVersionRef = serde_json::from_value(serde_json::json!({
        "workspace": scope,
        "file_id": file_id,
        "version_id": version_id,
        "byte_length": byte_length,
        "sha256": sha256,
    }))
    .map_err(|_| HttpError::invalid_request())?;
    Ok(SessionFileChunkReadRequest {
        download: SessionFileDownloadRequest {
            grant_id,
            upload_id,
            expected_file,
        },
        chunk_index: u32::try_from(chunk_index).map_err(|_| HttpError::invalid_request())?,
    })
}

/// All binary metadata is selected by the Source result, never reflected from query selectors.
pub(super) fn response_headers(chunk: &SessionFileChunkRead) -> Result<HeaderMap, HttpError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"chunk.bin\""),
    );
    headers.insert(
        "x-cyanrex-file-transfer-version",
        HeaderValue::from_static("1"),
    );
    let file = chunk.file();
    for (name, value) in [
        ("content-length", chunk.bytes().len().to_string()),
        ("x-cyanrex-file-id", file.file_id().to_string()),
        ("x-cyanrex-file-version-id", file.version_id().to_string()),
        ("x-cyanrex-chunk-index", chunk.chunk_index().to_string()),
        ("x-cyanrex-chunk-sha256", chunk.chunk_sha256().to_string()),
        ("x-cyanrex-file-sha256", file.sha256().to_string()),
    ] {
        let value = HeaderValue::from_str(&value).map_err(|_| {
            HttpError::dispatched(StatusCode::SERVICE_UNAVAILABLE, "service_unavailable")
        })?;
        headers.insert(name, value);
    }
    Ok(headers)
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
