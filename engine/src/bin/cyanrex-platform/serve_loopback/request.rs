//! Transport shape only; no forwarded metadata, peer identity or authorization inference.
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode, Version},
    middleware::Next,
    response::Response,
};

pub(super) async fn gate(
    State(authority): State<String>,
    request: Request,
    next: Next,
) -> Response {
    let mut hosts = request.headers().get_all(header::HOST).iter();
    let exact_host = hosts.next().map(HeaderValue::as_bytes) == Some(authority.as_bytes())
        && hosts.next().is_none();
    let upgrade = request.headers().contains_key(header::UPGRADE)
        || request
            .headers()
            .get_all(header::CONNECTION)
            .iter()
            .any(|value| {
                value
                    .as_bytes()
                    .split(|byte| *byte == b',')
                    .any(|token| token.trim_ascii().eq_ignore_ascii_case(b"upgrade"))
            });
    let uri = request.uri();
    if request.version() != Version::HTTP_11
        || request.method() == Method::CONNECT
        || !exact_host
        || upgrade
        || uri.scheme().is_some()
        || uri.authority().is_some()
        || !uri.path().starts_with('/')
    {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::BAD_REQUEST;
        let headers = response.headers_mut();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        return response;
    }
    next.run(request).await
}
