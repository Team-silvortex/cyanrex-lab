use crate::{
    platform_http::errors::HttpError, services::auth_service::durable_source::valid_token,
};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};

const SESSION_COOKIE: &str = "cyanrex_platform_session";
const MAX_COOKIE_BYTES: usize = 8 * 1024;

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a HeaderValue> {
    let mut values = headers.get_all(name).iter();
    let first = values.next()?;
    if values.next().is_some() {
        None
    } else {
        Some(first)
    }
}

/// Pre-authentication transport only. Neither legacy cookies nor forwarding headers grant trust.
pub(super) fn preflight(headers: &HeaderMap, origin: &str) -> Result<(), HttpError> {
    if single(headers, "x-cyanrex-platform-request").map(HeaderValue::as_bytes) != Some(b"1")
        || single(headers, "origin").map(HeaderValue::as_bytes) != Some(origin.as_bytes())
    {
        return Err(HttpError::new(StatusCode::FORBIDDEN, "request_forbidden"));
    }
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(HttpError::new(StatusCode::UNAUTHORIZED, "invalid_session"));
    }
    // Check the entire Cookie header budget before interpreting any field, not merely the first
    // occurrence or the prefix before a dedicated cookie. Unrelated legacy cookies are not adopted.
    let size = headers
        .get_all(header::COOKIE)
        .iter()
        .fold(0usize, |size, value| {
            size.saturating_add(value.as_bytes().len())
        });
    if size > MAX_COOKIE_BYTES {
        return Err(HttpError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
        ));
    }
    let mut present = false;
    let mut malformed = false;
    for value in headers.get_all(header::COOKIE) {
        malformed |= value.to_str().is_err();
        for pair in value.as_bytes().split(|byte| *byte == b';') {
            let end = pair
                .iter()
                .position(|byte| *byte == b'=')
                .unwrap_or(pair.len());
            // Values are irrelevant: even obs-text in a dedicated cookie cannot hide its name.
            // HeaderValue already rejects controls other than horizontal whitespace.
            present |= pair[..end].trim_ascii() == SESSION_COOKIE.as_bytes();
        }
    }
    if present {
        // Do not validate, clear, replace or silently ignore even an expired or malformed value.
        // Intentional account switching first needs a separately confirmed explicit logout.
        return Err(HttpError::new(StatusCode::CONFLICT, "session_present"));
    }
    if malformed {
        return Err(HttpError::invalid_request());
    }
    Ok(())
}

pub(super) fn issued_cookie(token: &str, origin: &str) -> Result<HeaderValue, HttpError> {
    let failed = || HttpError::dispatched(StatusCode::SERVICE_UNAVAILABLE, "service_unavailable");
    if !valid_token(token) {
        return Err(failed());
    }
    // Match the existing logout name/path/host-only policy without modifying its bytes. This
    // browser-session cookie is neither persistent storage nor proof of delivery to that browser.
    let secure = if origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let mut cookie = HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; Path=/platform/v1; HttpOnly; SameSite=Strict{secure}"
    ))
    .map_err(|_| failed())?;
    cookie.set_sensitive(true);
    Ok(cookie)
}
