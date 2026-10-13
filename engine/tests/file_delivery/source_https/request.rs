//! Parse only the native HTTP envelope, preserving the original raw selectors and header entries.
use super::{
    native_io::{self, Control},
    server::{Kind, ObservedRequest},
};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, Uri};
use std::{
    io::{self, Read},
    os::fd::AsRawFd,
    time::{Duration, Instant},
};

fn invalid() -> io::Error {
    // Never include raw requests, credentials, paths or parser diagnostics in fixture errors.
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid bounded Source TLS HTTP envelope",
    )
}
pub(super) fn read<R: Read + AsRawFd>(
    incoming: &mut R,
    pending: &mut Vec<u8>,
    control: &Control,
) -> io::Result<ObservedRequest> {
    let deadline = Instant::now() + Duration::from_secs(40);
    let end = loop {
        if let Some(offset) = pending.windows(4).position(|part| part == b"\r\n\r\n") {
            let end = offset + 4;
            if end > 8192 {
                return Err(invalid());
            }
            break end;
        }
        if pending.len() >= 8192 {
            return Err(invalid());
        }
        native_io::read_more(incoming, pending, control, deadline)?;
    };
    let text = std::str::from_utf8(&pending[..end]).map_err(|_| invalid())?;
    let mut lines = text.split("\r\n");
    let mut start = lines.next().ok_or_else(invalid)?.split_whitespace();
    let method = start
        .next()
        .ok_or_else(invalid)?
        .parse::<Method>()
        .map_err(|_| invalid())?;
    let uri = start
        .next()
        .ok_or_else(invalid)?
        .parse::<Uri>()
        .map_err(|_| invalid())?;
    if start.next() != Some("HTTP/1.1") || start.next().is_some() {
        return Err(invalid());
    }
    let mut headers = HeaderMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or_else(invalid)?;
        let name = name.parse::<HeaderName>().map_err(|_| invalid())?;
        let mut value =
            HeaderValue::from_str(value.trim_matches([' ', '\t'])).map_err(|_| invalid())?;
        if name == header::COOKIE || name == header::AUTHORIZATION {
            value.set_sensitive(true);
        }
        headers.append(name, value);
    }
    // The controlled client emits an empty native GET, never chunked request framing.
    if headers.contains_key(header::TRANSFER_ENCODING) {
        return Err(invalid());
    }
    let mut all_lengths = headers.get_all(header::CONTENT_LENGTH).iter();
    let length = match all_lengths.next() {
        Some(value) => value
            .to_str()
            .map_err(|_| invalid())?
            .parse::<usize>()
            .map_err(|_| invalid())?,
        None => 0,
    };
    if all_lengths.next().is_some() {
        return Err(invalid());
    }
    let full = end
        .checked_add(length)
        .filter(|length| *length <= 8192)
        .ok_or_else(invalid)?;
    while pending.len() < full {
        native_io::read_more(incoming, pending, control, deadline)?;
    }
    let body = pending[end..full].to_vec();
    pending.drain(..full);
    // Kind is only a recording/hold selector. Do not decode, replace or normalize URI query fields.
    let kind = match uri.path() {
        "/platform/v1/file-manifests" => Kind::Manifest,
        "/platform/v1/file-chunks" => {
            let index = uri
                .query()
                .ok_or_else(invalid)?
                .split('&')
                .find_map(|field| field.strip_prefix("chunk_index="))
                .ok_or_else(invalid)?
                .parse::<u32>()
                .map_err(|_| invalid())?;
            Kind::Chunk(index)
        }
        _ => return Err(invalid()),
    };
    Ok(ObservedRequest {
        method,
        uri,
        headers,
        body,
        kind,
        staged_before_request: Vec::new(),
    })
}
