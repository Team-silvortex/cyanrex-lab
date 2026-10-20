use super::super::test_support::*;
use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, HeaderValue, StatusCode},
};
use http_body::Frame;
use http_body_util::StreamBody;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn login_http_json_is_map_only_versioned_and_rejects_duplicate_unknown_or_invalid_fields() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    let mut invalid=vec!["".to_owned(),"null".to_owned(),"true".to_owned(),"{}".to_owned(),"[]".to_owned(),
        format!("[1,\"synthetic-user\",\"{PASSWORD}\",\"{OTP}\"]"),
        format!("{{\"schema_version\":1,\"username\":\"synthetic-user\",\"password\":\"{PASSWORD}\",\"password\":\"other\",\"otp\":\"{OTP}\"}}"),
        format!("{{\"schema_version\":1,\"schema_version\":1,\"username\":\"synthetic-user\",\"password\":\"{PASSWORD}\",\"otp\":\"{OTP}\"}}"),
        format!("{{\"schema_version\":1,\"username\":\"synthetic-user\",\"username\":\"other-user\",\"password\":\"{PASSWORD}\",\"otp\":\"{OTP}\"}}"),
        format!("{{\"schema_version\":1,\"username\":\"synthetic-user\",\"password\":\"{PASSWORD}\",\"otp\":\"{OTP}\",\"otp\":\"000000\"}}")];
    for (field, value) in [
        ("schema_version", json!(0)),
        ("schema_version", json!(2)),
        ("schema_version", json!("1")),
        ("schema_version", json!(1.0)),
        ("username", json!("Synthetic-user")),
        ("username", json!(" synthetic-user ")),
        ("username", json!("xy")),
        ("username", json!("x".repeat(65))),
        ("username", json!(null)),
        ("password", json!(null)),
        ("otp", json!(123456)),
        ("password", json!("x".repeat(4097))),
        ("password", json!("é".repeat(2049))),
        ("otp", json!("0".repeat(65))),
        ("token", json!(TOKEN)),
        ("authority_id", json!("synthetic")),
        ("role", json!("teacher")),
    ] {
        let mut value_object = valid_json();
        value_object[field] = value;
        invalid.push(value_object.to_string());
    }
    for field in ["schema_version", "username", "password", "otp"] {
        let mut value = valid_json();
        value.as_object_mut().unwrap().remove(field);
        invalid.push(value.to_string());
    }
    for value in invalid {
        error(
            &capture(app.clone(), request("POST", LOGIN, Body::from(value))).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn login_http_raw_password_and_otp_reach_one_backend_call_without_trimming_or_normalizing() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    for (password, otp) in [
        (
            "\t synthetic password\n".to_owned(),
            " 123456 \t".to_owned(),
        ),
        ("x".repeat(4096), "0".repeat(64)),
        ("\0".repeat(4096), "".to_owned()),
        ("".to_owned(), "".to_owned()),
    ] {
        let body = Body::from(
            json!({"schema_version":1,"username":"synthetic-user","password":password,"otp":otp})
                .to_string(),
        );
        success(
            &capture(app.clone(), request("POST", LOGIN, body)).await,
            true,
        );
        let received = backend.take_received();
        assert_eq!(received.len(), 1);
        assert!(received[0].username == "synthetic-user");
        assert!(
            received[0].password == password,
            "password must not be normalized by the adapter"
        );
        assert!(
            received[0].otp == otp,
            "OTP must not be normalized by the adapter"
        );
    }
    assert_eq!(backend.calls(), 4);
}

#[tokio::test]
async fn login_http_wire_budget_counts_actual_streamed_bytes_not_content_length() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    let mut exact = valid_json().to_string();
    exact.push_str(&" ".repeat(32 * 1024 - exact.len()));
    success(
        &capture(
            app.clone(),
            request("POST", LOGIN, Body::from(exact.clone())),
        )
        .await,
        true,
    );
    exact.push(' ');
    let bytes = Bytes::from(exact);
    let mut req = request(
        "POST",
        LOGIN,
        Body::from_stream(futures_util::stream::iter([
            Ok::<_, std::io::Error>(bytes.slice(..16000)),
            Ok(bytes.slice(16000..)),
        ])),
    );
    req.headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
    error(
        &capture(app, req).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn login_http_encoding_media_type_and_duplicate_headers_fail_without_polling_body() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    for media in [
        None,
        Some("text/plain"),
        Some("application/json; charset=UTF-8"),
        Some("application/json; charset=utf-8; boundary=x"),
    ] {
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut().remove(header::CONTENT_TYPE);
        if let Some(media) = media {
            req.headers_mut()
                .insert(header::CONTENT_TYPE, media.parse().unwrap());
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "not_attempted",
        );
    }
    let mut req = request("POST", LOGIN, untouched());
    req.headers_mut().append(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        "not_attempted",
    );
    for encoding in ["", "identity", "gzip"] {
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut()
            .insert(header::CONTENT_ENCODING, encoding.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
    let mut req = request("POST", LOGIN, valid_body());
    req.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    success(&capture(app, req).await, true);
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn login_http_body_errors_trailers_and_non_json_bytes_never_dispatch_login() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    let body = Body::new(StreamBody::new(futures_util::stream::iter([
        Ok::<Frame<Bytes>, std::io::Error>(Frame::data(Bytes::from(valid_json().to_string()))),
        Ok(Frame::trailers(HeaderMap::new())),
    ])));
    error(
        &capture(app.clone(), request("POST", LOGIN, body)).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    let body = Body::from_stream(futures_util::stream::iter([Err::<Bytes, _>(
        std::io::Error::other("synthetic incoming failure"),
    )]));
    error(
        &capture(app.clone(), request("POST", LOGIN, body)).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    for bytes in [vec![0xff], vec![0xef, 0xbb, 0xbf, b'{', b'}']] {
        error(
            &capture(app.clone(), request("POST", LOGIN, Body::from(bytes))).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn login_http_whole_body_timeout_covers_stalled_and_infinite_ready_empty_frames() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    let bodies = [
        Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>()),
        Body::from_stream(futures_util::stream::repeat_with(|| {
            Ok::<Bytes, std::io::Error>(Bytes::new())
        })),
    ];
    for body in bodies {
        let response = tokio::time::timeout(
            Duration::from_secs(8),
            capture(app.clone(), request("POST", LOGIN, body)),
        )
        .await
        .expect("fixed whole-body deadline was evaded");
        error(
            &response,
            StatusCode::REQUEST_TIMEOUT,
            "request_timeout",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}
