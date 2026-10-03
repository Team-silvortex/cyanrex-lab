use super::{errors::HttpError, TaskContentHttpConfigError};
use axum::{
    body::{Body, Bytes, HttpBody},
    extract::Request,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{future::poll_fn, net::IpAddr, pin::Pin, time::Duration};
use uuid::Uuid;

const SESSION_COOKIE: &str = "cyanrex_platform_session";
const MAX_COOKIE_BYTES: usize = 8 * 1024;
const MAX_REQUEST_BYTES: usize = 64 * 1024;

pub(super) fn validate_origin(value: &str) -> Result<(), TaskContentHttpConfigError> {
    let invalid = TaskContentHttpConfigError::InvalidOrigin;
    if value.len() > 2048 {
        return Err(invalid);
    }
    let url = reqwest::Url::parse(value).map_err(|_| invalid)?;
    if url.origin().ascii_serialization() != value
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid);
    }
    let host = url.host_str().ok_or(invalid)?;
    if host.contains('*') {
        return Err(invalid);
    }
    let loopback = host == "localhost"
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if url.scheme() == "https" || (url.scheme() == "http" && loopback) {
        Ok(())
    } else {
        Err(invalid)
    }
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a HeaderValue> {
    let mut all = headers.get_all(name).iter();
    let first = all.next()?;
    if all.next().is_some() {
        None
    } else {
        Some(first)
    }
}

/// Header selection is not authorization: only the transaction-owned Session command authorizes.
/// No raw token appears in Debug, error payloads, logs, request query or response cookies.
pub(super) fn credentials(
    headers: &HeaderMap,
    method: &Method,
    origin: &str,
) -> Result<String, HttpError> {
    let forbidden = || HttpError::new(StatusCode::FORBIDDEN, "request_forbidden");
    if single(headers, "x-cyanrex-platform-request").map(HeaderValue::as_bytes) != Some(b"1") {
        return Err(forbidden());
    }
    if method != Method::GET || headers.contains_key(header::ORIGIN) {
        if single(headers, "origin").map(HeaderValue::as_bytes) != Some(origin.as_bytes()) {
            return Err(forbidden());
        }
    }
    let unauthorized = || HttpError::new(StatusCode::UNAUTHORIZED, "invalid_session");
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(unauthorized());
    }
    let mut size = 0usize;
    let mut token = None;
    for value in headers.get_all(header::COOKIE) {
        size = size.saturating_add(value.as_bytes().len());
        if size > MAX_COOKIE_BYTES {
            return Err(HttpError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_too_large",
            ));
        }
        for pair in value.to_str().map_err(|_| unauthorized())?.split(';') {
            let pair = pair.trim_matches([' ', '\t']);
            let Some((name, value)) = pair.split_once('=') else {
                if pair == SESSION_COOKIE {
                    return Err(unauthorized());
                }
                continue;
            };
            if name.trim_matches([' ', '\t']) != SESSION_COOKIE {
                continue;
            }
            let value = value.trim_matches([' ', '\t']);
            if token.is_some()
                || value.len() != 36
                || !Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
            {
                return Err(unauthorized());
            }
            token = Some(value.to_owned());
        }
    }
    token.ok_or_else(unauthorized)
}

pub(super) async fn request_bytes(request: Request, empty: bool) -> Result<Bytes, HttpError> {
    let headers = request.headers();
    if headers.contains_key(header::CONTENT_ENCODING) {
        return Err(HttpError::invalid_request());
    }
    if (!empty || headers.contains_key(header::CONTENT_TYPE))
        && !matches!(
            single(headers, "content-type").map(HeaderValue::as_bytes),
            Some(b"application/json" | b"application/json; charset=utf-8")
        )
    {
        return Err(HttpError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        ));
    }
    let limit = if empty { 0 } else { MAX_REQUEST_BYTES };
    // Bound actual streamed bytes, not an untrusted Content-Length. This deadline ends BEFORE
    // dispatch; it never cancels admitted mutations or claims rollback of their transactions.
    tokio::time::timeout(
        Duration::from_secs(2),
        collect_body(request.into_body(), limit),
    )
    .await
    .map_err(|_| HttpError::new(StatusCode::REQUEST_TIMEOUT, "request_timeout"))?
}

async fn collect_body(mut body: Body, limit: usize) -> Result<Bytes, HttpError> {
    let mut bytes = Vec::new();
    while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
        let chunk = frame
            .map_err(|_| HttpError::invalid_request())?
            .into_data()
            .map_err(|_| HttpError::invalid_request())?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(if limit == 0 {
                HttpError::invalid_request()
            } else {
                HttpError::new(StatusCode::PAYLOAD_TOO_LARGE, "request_too_large")
            });
        }
        bytes.extend_from_slice(&chunk);
        // A stream of immediately-ready empty frames must also yield, so it cannot starve
        // the timer and evade the whole-body deadline without spending its byte budget.
        tokio::task::yield_now().await;
    }
    Ok(Bytes::from(bytes))
}

pub(super) async fn response_headers(request: Request, next: Next) -> Response {
    let head = request.method() == Method::HEAD;
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if head {
        *response.body_mut() = Body::empty();
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
    }
    response
}
