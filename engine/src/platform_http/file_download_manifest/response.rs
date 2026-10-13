use crate::{
    models::collaboration::{FileTransferManifest, MAX_FILE_TRANSFER_MANIFEST_BYTES},
    platform_http::errors::HttpError,
};
use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
    response::Response,
};

fn unavailable() -> HttpError {
    HttpError::dispatched(StatusCode::SERVICE_UNAVAILABLE, "service_unavailable")
}

fn checked_bytes(encoded: Vec<u8>) -> Result<Vec<u8>, HttpError> {
    if encoded.len() > MAX_FILE_TRANSFER_MANIFEST_BYTES {
        return Err(unavailable());
    }
    Ok(encoded)
}

/// Direct server-selected typed manifest, with no envelope or authorization/filename headers.
pub(super) fn manifest(manifest: &FileTransferManifest) -> Result<Response, HttpError> {
    // Typed fields already bound this representation. This post-serialization check is not
    // fallible-allocation support, a byte-proof, a global RAM quota or a new body deadline.
    let encoded = checked_bytes(serde_json::to_vec(manifest).map_err(|_| unavailable())?)?;
    let length = HeaderValue::from_str(&encoded.len().to_string()).map_err(|_| unavailable())?;
    let mut response = Response::new(Body::from(encoded));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, length);
    Ok(response)
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
