//! Explicit, fixed same-origin Session page; never a directory server or teaching application.
use axum::{
    body::{Body, HttpBody},
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use std::{future::poll_fn, pin::Pin, sync::Arc, time::Duration};

const POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src data:; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; worker-src 'none'";

pub(super) fn routes(origin: &str) -> Router {
    // Private CLI composition already checked this exact Origin against scheme/literal bind.
    // Closed compile-time assets only: no request-selected path, disk root, renderer or proxy.
    Router::new()
        .route("/platform/", any(dispatch))
        .route("/platform/ui/app.js", any(dispatch))
        .route("/platform/ui/labels.js", any(dispatch))
        .route("/platform/ui/style.css", any(dispatch))
        .route("/platform/ui/tasks.js", any(dispatch))
        .route("/platform/ui/task-contract.js", any(dispatch))
        .route("/platform/ui/task-edit.js", any(dispatch))
        .layer(middleware::from_fn(response_headers))
        .with_state(Arc::<str>::from(origin))
}

fn asset(path: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match path {
        "/platform/" => (
            "text/html; charset=utf-8",
            include_str!("session_ui/index.html"),
            "document",
        ),
        "/platform/ui/app.js" => (
            "text/javascript; charset=utf-8",
            include_str!("session_ui/app.js"),
            "script",
        ),
        "/platform/ui/labels.js" => (
            "text/javascript; charset=utf-8",
            include_str!("session_ui/labels.js"),
            "script",
        ),
        "/platform/ui/style.css" => (
            "text/css; charset=utf-8",
            include_str!("session_ui/style.css"),
            "style",
        ),
        "/platform/ui/tasks.js" => (
            "text/javascript; charset=utf-8",
            include_str!("session_ui/tasks.js"),
            "script",
        ),
        "/platform/ui/task-contract.js" => (
            "text/javascript; charset=utf-8",
            include_str!("session_ui/task-contract.js"),
            "script",
        ),
        "/platform/ui/task-edit.js" => (
            "text/javascript; charset=utf-8",
            include_str!("session_ui/task-edit.js"),
            "script",
        ),
        _ => return None,
    })
}

fn optional_single<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a [u8]>, ()> {
    let mut values = headers.get_all(name).iter();
    let value = values.next();
    if values.next().is_some() {
        return Err(());
    }
    Ok(value.map(HeaderValue::as_bytes))
}

fn preflight(headers: &HeaderMap, origin: &str, dest: &str) -> Result<(), ()> {
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(());
    }
    if optional_single(headers, "origin")?.is_some_and(|value| value != origin.as_bytes()) {
        return Err(());
    }
    if let Some(site) = optional_single(headers, "sec-fetch-site")? {
        if site != b"same-origin" && !(dest == "document" && site == b"none") {
            return Err(());
        }
    }
    if optional_single(headers, "sec-fetch-dest")?.is_some_and(|value| value != dest.as_bytes()) {
        return Err(());
    }
    if let Some(mode) = optional_single(headers, "sec-fetch-mode")? {
        if if dest == "document" {
            mode != b"navigate"
        } else {
            mode != b"cors" && mode != b"no-cors"
        } {
            return Err(());
        }
    }
    // Metadata is only an additional embedding guard. Missing metadata allows native probes;
    // it is not authentication, and the unchanged API checks still authorize every command.
    Ok(())
}

fn error(status: StatusCode, code: &'static str) -> Response {
    let mut response = (
        status,
        Json(
            serde_json::json!({"schema_version":1,"error":{"code":code,"outcome":"not_attempted"}}),
        ),
    )
        .into_response();
    if status == StatusCode::METHOD_NOT_ALLOWED {
        response
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET"));
    }
    response
}

async fn empty_body(mut body: Body) -> Result<(), ()> {
    while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
        let data = frame.map_err(|_| ())?.into_data().map_err(|_| ())?;
        if !data.is_empty() {
            return Err(());
        }
        tokio::task::yield_now().await;
    }
    Ok(())
}

async fn dispatch(State(origin): State<Arc<str>>, request: Request) -> Response {
    let Some((mime, content, dest)) = asset(request.uri().path()) else {
        return error(StatusCode::NOT_FOUND, "not_found");
    };
    if request.method() != Method::GET {
        return error(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
    }
    if request.uri().query().is_some() {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    if preflight(request.headers(), &origin, dest).is_err() {
        return error(StatusCode::FORBIDDEN, "request_forbidden");
    }
    if request.headers().contains_key(header::CONTENT_ENCODING)
        || request.headers().contains_key(header::CONTENT_TYPE)
    {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    match tokio::time::timeout(Duration::from_secs(2), empty_body(request.into_body())).await {
        Ok(Ok(())) => ([(header::CONTENT_TYPE, mime)], content).into_response(),
        Ok(Err(())) => error(StatusCode::BAD_REQUEST, "invalid_request"),
        Err(_) => error(StatusCode::REQUEST_TIMEOUT, "request_timeout"),
    }
}

async fn response_headers(request: Request, next: Next) -> Response {
    let head = request.method() == Method::HEAD;
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (key, value) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("content-security-policy", POLICY),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-resource-policy", "same-origin"),
        ("cross-origin-opener-policy", "same-origin"),
        ("x-frame-options", "DENY"),
    ] {
        headers.insert(key, HeaderValue::from_static(value));
    }
    if head {
        *response.body_mut() = Body::empty();
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
    }
    response
}

#[cfg(test)]
#[path = "session_ui/tests.rs"]
mod tests;
