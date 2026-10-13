//! Fixed metadata observation only; no file dispatch or reusable byte permission is minted.
use super::{
    manifest_contract, manifest_inputs_valid, manifest_receive, FileGrantId,
    FileManifestClientError, FileTransferManifest, FileUploadId, FileVersionRef, Inner,
};
use reqwest::header::{HeaderValue, ACCEPT, ACCEPT_ENCODING, COOKIE};
use tokio::time::{timeout_at, Instant};

pub(super) async fn fetch(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    expected_file: &FileVersionRef,
) -> Result<FileTransferManifest, FileManifestClientError> {
    if !manifest_inputs_valid(inner, token, expected_file) {
        return Err(FileManifestClientError::InvalidInput);
    }
    let _permit = inner
        .manifests
        .clone()
        .try_acquire_owned()
        .map_err(|_| FileManifestClientError::CapacityExhausted)?;
    let mut endpoint = inner.manifest_endpoint.clone();
    endpoint.set_query(Some(&format!(
        "grant_id={grant_id}&upload_id={upload_id}&file_id={}&version_id={}&byte_length={}&sha256={}",
        expected_file.file_id(), expected_file.version_id(), expected_file.byte_length(),
        expected_file.sha256()
    )));
    let mut cookie = HeaderValue::from_str(&format!("cyanrex_platform_session={token}"))
        .map_err(|_| FileManifestClientError::InvalidInput)?;
    cookie.set_sensitive(true);
    let request = inner
        .http
        .get(endpoint)
        .header(COOKIE, cookie)
        .header("x-cyanrex-platform-request", "1")
        .header(ACCEPT, "application/json")
        .header(ACCEPT_ENCODING, "identity");
    // Native GET has no Origin/body/Authorization. All diagnostics remain static and sanitized.
    let deadline = Instant::now() + inner.header_timeout;
    let response = timeout_at(deadline, request.send())
        .await
        .map_err(|_| FileManifestClientError::TimedOut)?
        .map_err(|error| {
            if error.is_timeout() {
                FileManifestClientError::TimedOut
            } else {
                FileManifestClientError::Unavailable
            }
        })?;
    let body_started = Instant::now();
    if body_started >= deadline {
        return Err(FileManifestClientError::TimedOut);
    }
    // Keep the actual framed message body, including error/trailer frames. This is not TCP EOF.
    let response: axum::http::Response<reqwest::Body> = response.into();
    let length = manifest_contract::validate(&response)?;
    #[cfg(test)]
    let response = response.map(|body| {
        super::work_tests::ObservedBody::new(body, inner.manifest_eof_observer.clone())
    });
    manifest_receive::collect(
        response.into_body(),
        length,
        expected_file,
        inner.idle_timeout,
        inner.total_timeout,
        body_started,
    )
    .await
}
