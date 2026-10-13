use crate::{
    models::collaboration::{
        FileGrantId, FileId, FileUploadId, FileVersionId, FileVersionRef, Sha256Digest,
        WorkspaceRef, MAX_FILE_TRANSFER_BYTES,
    },
    platform_http::errors::HttpError,
    services::auth_service::durable_source::SessionFileDownloadRequest,
};

const MAX_QUERY_BYTES: usize = 512;

/// Exact raw selectors only. Trusted composition, not request data, supplies the workspace.
pub(super) fn download_request(
    query: Option<&str>,
    scope: WorkspaceRef,
) -> Result<SessionFileDownloadRequest, HttpError> {
    let query = query.ok_or_else(HttpError::invalid_request)?;
    if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.contains(['%', '+']) {
        return Err(HttpError::invalid_request());
    }
    let mut fields = [None; 6];
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
    let length = field(4)?;
    if (length.len() > 1 && length.starts_with('0'))
        || !length.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HttpError::invalid_request());
    }
    let byte_length: u64 = length.parse().map_err(|_| HttpError::invalid_request())?;
    if byte_length > MAX_FILE_TRANSFER_BYTES {
        return Err(HttpError::invalid_request());
    }
    let sha256: Sha256Digest = field(5)?
        .parse()
        .map_err(|_| HttpError::invalid_request())?;
    // This small host-created object follows strict raw duplicate/lexical checks. It is not
    // decoded client JSON, and cannot erase duplicate selectors or choose owner/scope/storage.
    let expected_file: FileVersionRef = serde_json::from_value(serde_json::json!({
        "workspace": scope, "file_id": file_id, "version_id": version_id,
        "byte_length": byte_length, "sha256": sha256,
    }))
    .map_err(|_| HttpError::invalid_request())?;
    Ok(SessionFileDownloadRequest {
        grant_id,
        upload_id,
        expected_file,
    })
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
