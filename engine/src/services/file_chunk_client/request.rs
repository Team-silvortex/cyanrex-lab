use super::{
    FileChunkClientError, FileGrantId, FileTransferManifest, FileUploadId, Inner, ReceivedFileChunk,
};
use reqwest::header::{HeaderValue, ACCEPT, ACCEPT_ENCODING, COOKIE};
use tokio::time::{timeout_at, Instant};
use uuid::Uuid;

pub(super) async fn fetch(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    manifest: &FileTransferManifest,
    index: u32,
) -> Result<ReceivedFileChunk, FileChunkClientError> {
    if manifest.workspace() != inner.scope
        || manifest.chunk(index).is_err()
        || token.len() != 36
        || !Uuid::parse_str(token).is_ok_and(|id| !id.is_nil() && id.to_string() == token)
    {
        return Err(FileChunkClientError::InvalidInput);
    }
    let permit = inner
        .controls
        .clone()
        .try_acquire_owned()
        .map_err(|_| FileChunkClientError::CapacityExhausted)?;
    let mut endpoint = inner.endpoint.clone();
    endpoint.set_query(Some(&format!(
        "grant_id={grant_id}&upload_id={upload_id}&file_id={}&version_id={}&byte_length={}&sha256={}&chunk_index={index}",
        manifest.file_id(), manifest.version_id(), manifest.byte_length(), manifest.sha256()
    )));
    let mut cookie = HeaderValue::from_str(&format!("cyanrex_platform_session={token}"))
        .map_err(|_| FileChunkClientError::InvalidInput)?;
    cookie.set_sensitive(true);
    let request = inner
        .http
        .get(endpoint)
        .header(COOKIE, cookie)
        .header("x-cyanrex-platform-request", "1")
        .header(ACCEPT, "application/octet-stream")
        .header(ACCEPT_ENCODING, "identity");
    // Native GET deliberately omits Origin: endpoint origin need not equal the server's UI origin.
    // No request body, Authorization, query credential, logging or remote diagnostic is supplied.
    let deadline = Instant::now() + inner.header_timeout;
    let response = timeout_at(deadline, request.send())
        .await
        .map_err(|_| FileChunkClientError::TimedOut)?
        .map_err(|error| {
            if error.is_timeout() {
                FileChunkClientError::TimedOut
            } else {
                FileChunkClientError::Unavailable
            }
        })?;
    if Instant::now() >= deadline {
        return Err(FileChunkClientError::TimedOut);
    }
    // Preserve the actual framed body, including errors/trailers, instead of convenience chunk()
    // or bytes() collectors. The header timer has ended; Receiver owns idle/total from here.
    let response: axum::http::Response<reqwest::Body> = response.into();
    #[cfg(test)]
    let response =
        response.map(|body| super::work_tests::ObservedBody::new(body, inner.eof_observer.clone()));
    inner
        .receiver
        .receive_and_stage_leased(manifest, index, response, Some(permit))
        .await
        .map_err(FileChunkClientError::Receive)
}
