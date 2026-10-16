use crate::{
    models::collaboration::{
        FileGrantId, FileId, FileUploadId, FileVersionId, FileVersionRef, Sha256Digest,
        WorkspaceRef, MAX_FILE_TRANSFER_BYTES,
    },
    platform_http::{errors::HttpError, file_selectors},
    services::auth_service::durable_source::SessionFileDownloadRequest,
};

/// Exact raw selectors only. Trusted composition, not request data, supplies the workspace.
pub(super) fn download_request(
    query: Option<&str>,
    scope: WorkspaceRef,
) -> Result<SessionFileDownloadRequest, HttpError> {
    let [grant_id, upload_id, file_id, version_id, length, sha256] = file_selectors::parse(
        query,
        [
            "grant_id",
            "upload_id",
            "file_id",
            "version_id",
            "byte_length",
            "sha256",
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
    let byte_length = file_selectors::decimal(length)?;
    if byte_length > MAX_FILE_TRANSFER_BYTES {
        return Err(HttpError::invalid_request());
    }
    let sha256: Sha256Digest = sha256.parse().map_err(|_| HttpError::invalid_request())?;
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
