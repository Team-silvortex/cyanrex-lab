use crate::{
    models::collaboration::{
        FileGrantId, FileId, FileUploadId, FileVersionId, FileVersionRef, Sha256Digest,
        WorkspaceRef, MAX_FILE_TRANSFER_BYTES, MAX_FILE_TRANSFER_CHUNKS,
    },
    platform_http::errors::HttpError,
    services::auth_service::durable_source::{
        SessionFileChunkRead, SessionFileChunkReadRequest, SessionFileDownloadRequest,
    },
};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};

const MAX_QUERY_BYTES: usize = 512;

/// Raw canonical selectors only. Scope and owner attribution never come from the request.
pub(super) fn chunk_request(
    query: Option<&str>,
    scope: WorkspaceRef,
) -> Result<SessionFileChunkReadRequest, HttpError> {
    let query = query.ok_or_else(HttpError::invalid_request)?;
    if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.contains(['%', '+']) {
        return Err(HttpError::invalid_request());
    }
    let mut fields = [None; 7];
    for pair in query.split('&') {
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(HttpError::invalid_request)?;
        if value.is_empty() || value.contains('=') {
            return Err(HttpError::invalid_request());
        }
        let index = match name {
            "grant_id" => 0,
            "upload_id" => 1,
            "file_id" => 2,
            "version_id" => 3,
            "byte_length" => 4,
            "sha256" => 5,
            "chunk_index" => 6,
            _ => return Err(HttpError::invalid_request()),
        };
        if fields[index].replace(value).is_some() {
            return Err(HttpError::invalid_request());
        }
    }
    let field = |index: usize| fields[index].ok_or_else(HttpError::invalid_request);
    let grant_id: FileGrantId = field(0)?
        .parse()
        .map_err(|_| HttpError::invalid_request())?;
    let upload_id: FileUploadId = field(1)?
        .parse()
        .map_err(|_| HttpError::invalid_request())?;
    let file_id: FileId = field(2)?
        .parse()
        .map_err(|_| HttpError::invalid_request())?;
    let version_id: FileVersionId = field(3)?
        .parse()
        .map_err(|_| HttpError::invalid_request())?;
    let byte_length = decimal(field(4)?)?;
    let sha256: Sha256Digest = field(5)?
        .parse()
        .map_err(|_| HttpError::invalid_request())?;
    let chunk_index = decimal(field(6)?)?;
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

fn decimal(value: &str) -> Result<u64, HttpError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HttpError::invalid_request());
    }
    value.parse().map_err(|_| HttpError::invalid_request())
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
