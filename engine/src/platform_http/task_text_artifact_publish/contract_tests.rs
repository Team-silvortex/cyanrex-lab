use super::{test_support::*, Backend as PublishBackend, TaskTextArtifactPublishHttpState};
use crate::{
    models::collaboration::WorkspaceRef, platform_http::build_task_text_artifact_publish_router,
};
use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, HeaderValue, StatusCode},
};
use serde_json::{json, Value};
use uuid::Uuid;

async fn invalid(app: axum::Router, raw: impl Into<Bytes>) {
    error(
        &capture(app, request("POST", CREATE, Body::from(raw.into()))).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
}

fn raw_record(path: &str, replacement: &str) -> String {
    let mut changed = wire();
    *changed.pointer_mut(path).unwrap() = json!("__raw_record_placeholder__");
    let encoded = changed.to_string();
    let placeholder = json!("__raw_record_placeholder__").to_string();
    assert_eq!(encoded.matches(&placeholder).count(), 1);
    encoded.replacen(&placeholder, replacement, 1)
}

#[tokio::test]
async fn task_text_publish_top_record_requires_exact_five_fields_without_client_authority() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let valid = wire();
    let raw = valid.to_string();
    for value in [
        "null".into(),
        "[]".into(),
        "{}".into(),
        "\"text\"".into(),
        format!("[1,{},\"{ID}\",\"{REVISION}\",\"text\"]", valid["expected"]),
        format!("\u{feff}{raw}"),
        format!("{raw} {{}}"),
    ] {
        invalid(app.clone(), value).await;
    }
    for key in [
        "schema_version",
        "expected",
        "artifact_id",
        "revision_id",
        "text",
    ] {
        let mut changed = valid.clone();
        changed.as_object_mut().unwrap().remove(key);
        invalid(app.clone(), changed.to_string()).await;
        let mut changed = valid.clone();
        changed[key] = Value::Null;
        invalid(app.clone(), changed.to_string()).await;
        invalid(
            app.clone(),
            raw.replacen(
                &format!("\"{key}\":"),
                &format!("\"{key}\":{},\"{key}\":", valid[key]),
                1,
            ),
        )
        .await;
    }
    for key in [
        "title",
        "owner",
        "scope",
        "workspace",
        "namespace",
        "path",
        "sha256",
        "kind",
        "media_type",
        "filename",
        "language",
        "parent",
        "sequence",
        "task_id",
        "expected_revision",
        "run",
    ] {
        let mut changed = valid.clone();
        changed[key] = json!("not client authority");
        invalid(app.clone(), changed.to_string()).await;
    }
    for version in [json!(0), json!(2), json!(1.0), json!("1"), json!(-1)] {
        let mut changed = valid.clone();
        changed["schema_version"] = version;
        invalid(app.clone(), changed.to_string()).await;
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn task_text_publish_every_nested_record_rejects_arrays_unknown_and_duplicate_fields() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let valid = wire();
    let records: &[(&str, &[&str])] = &[
        ("/expected", &["task", "manifest"]),
        (
            "/expected/task",
            &[
                "schema_version",
                "reference",
                "owner",
                "title",
                "definition",
                "input_refs",
                "status",
                "revision",
                "created_at",
                "updated_at",
            ],
        ),
        ("/expected/task/reference", &["workspace", "task_id"]),
        (
            "/expected/task/reference/workspace",
            &["authority_id", "workspace_id"],
        ),
        ("/expected/task/owner", &["authority_id", "principal_id"]),
        (
            "/expected/task/input_refs/0",
            &["workspace", "artifact_id", "revision_id", "sha256"],
        ),
        (
            "/expected/task/input_refs/0/workspace",
            &["authority_id", "workspace_id"],
        ),
        (
            "/expected/manifest",
            &["schema_version", "title", "payload"],
        ),
        (
            "/expected/manifest/payload/0",
            &["kind", "filename", "language", "artifact"],
        ),
        (
            "/expected/manifest/payload/0/artifact",
            &["workspace", "artifact_id", "revision_id", "sha256"],
        ),
        (
            "/expected/manifest/payload/0/artifact/workspace",
            &["authority_id", "workspace_id"],
        ),
    ];
    for (path, keys) in records {
        let record = valid.pointer(path).unwrap();
        let raw = record.to_string();
        let array = Value::Array(keys.iter().map(|key| record[*key].clone()).collect());
        invalid(app.clone(), raw_record(path, &array.to_string())).await;
        let mut unknown = record.clone();
        unknown["unrequested"] = json!(true);
        invalid(app.clone(), raw_record(path, &unknown.to_string())).await;
        let first = keys[0];
        let duplicate = format!("{{\"{first}\":{},{}", record[first], &raw[1..]);
        invalid(app.clone(), raw_record(path, &duplicate)).await;
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn task_text_publish_requires_definition_null_and_known_status_string_without_granting_draft_authority(
) {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let mut missing = wire();
    missing["expected"]["task"]
        .as_object_mut()
        .unwrap()
        .remove("definition");
    invalid(app.clone(), missing.to_string()).await;
    for definition in [
        json!({}),
        json!([]),
        json!({"name":"claimed definition"}),
        json!("null"),
    ] {
        let mut changed = wire();
        changed["expected"]["task"]["definition"] = definition;
        invalid(app.clone(), changed.to_string()).await;
    }
    for status in [
        json!(null),
        json!("accepted"),
        json!("Draft"),
        json!({"draft":null}),
        json!(["draft"]),
        json!(1),
    ] {
        let mut changed = wire();
        changed["expected"]["task"]["status"] = status;
        invalid(app.clone(), changed.to_string()).await;
    }
    assert_eq!(backend.calls(), 0);
    // Parsing a historical known status is not proof of the current Task's status. Only
    // the real connector can establish it; this synthetic transport backend grants nothing.
    for status in [
        "draft",
        "ready",
        "in_progress",
        "blocked",
        "in_review",
        "cancelled",
    ] {
        let mut changed = wire();
        changed["expected"]["task"]["status"] = json!(status);
        assert_eq!(
            capture(
                app.clone(),
                request("POST", CREATE, Body::from(changed.to_string()))
            )
            .await
            .status,
            StatusCode::CREATED
        );
    }
    assert_eq!(backend.calls(), 6);
}

#[tokio::test]
async fn task_text_publish_path_scope_typed_scalars_and_mandatory_task_fields_validate_before_dispatch(
) {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for (path, value) in [
        ("/expected/task/reference/task_id", json!(ID)),
        (
            "/expected/task/reference/workspace/workspace_id",
            json!(Uuid::from_u128(8888)),
        ),
        (
            "/expected/task/owner/authority_id",
            json!(Uuid::from_u128(8888)),
        ),
        ("/expected/task/revision", json!(0)),
        ("/expected/task/revision", json!(7.0)),
        ("/expected/task/schema_version", json!(2)),
        ("/expected/manifest/schema_version", json!(2)),
        ("/expected/task/created_at", json!("not-a-time")),
        ("/expected/task/updated_at", json!(3)),
        ("/expected/task/input_refs/0/sha256", json!("A".repeat(64))),
    ] {
        let mut changed = wire();
        *changed.pointer_mut(path).unwrap() = value;
        invalid(app.clone(), changed.to_string()).await;
    }
    for key in [
        "schema_version",
        "reference",
        "owner",
        "title",
        "input_refs",
        "status",
        "revision",
        "created_at",
        "updated_at",
    ] {
        let mut changed = wire();
        changed["expected"]["task"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        invalid(app.clone(), changed.to_string()).await;
    }
    for key in ["artifact_id", "revision_id"] {
        for value in [
            "not-an-id".into(),
            ID.to_uppercase(),
            ID.replace('-', ""),
            "00000000-0000-0000-0000-000000000000".into(),
        ] {
            let mut changed = wire();
            changed[key] = json!(value);
            invalid(app.clone(), changed.to_string()).await;
        }
    }
    let configured = TaskTextArtifactPublishHttpState::configured(
        PublishBackend::Test(backend.clone()),
        ORIGIN,
        WorkspaceRef {
            workspace_id: Uuid::from_u128(8888).try_into().unwrap(),
            ..scope()
        },
        4,
    )
    .unwrap();
    invalid(
        build_task_text_artifact_publish_router(configured),
        wire().to_string(),
    )
    .await;
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn task_text_publish_complete_manifest_must_match_task_without_inline_bytes_or_execution_hints(
) {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for (path, value) in [
        ("/expected/manifest/title", json!("Different title")),
        ("/expected/task/input_refs", json!([])),
        ("/expected/manifest/payload", json!([])),
        (
            "/expected/manifest/payload/0/artifact/sha256",
            json!("b".repeat(64)),
        ),
        ("/expected/manifest/payload/0/kind", json!("code")),
        (
            "/expected/manifest/payload/0/filename",
            json!("../private.txt"),
        ),
        (
            "/expected/manifest/payload/0/language",
            json!("execute-me!"),
        ),
    ] {
        let mut changed = wire();
        *changed.pointer_mut(path).unwrap() = value;
        invalid(app.clone(), changed.to_string()).await;
    }
    let mut repeated = wire();
    let item = repeated["expected"]["manifest"]["payload"][0].clone();
    repeated["expected"]["manifest"]["payload"]
        .as_array_mut()
        .unwrap()
        .push(item.clone());
    repeated["expected"]["task"]["input_refs"]
        .as_array_mut()
        .unwrap()
        .push(item["artifact"].clone());
    invalid(app.clone(), repeated.to_string()).await;
    assert_eq!(backend.calls(), 0);
    let mut empty = wire();
    empty["expected"]["task"]["input_refs"] = json!([]);
    empty["expected"]["manifest"]["payload"] = json!([]);
    assert_eq!(
        capture(app, request("POST", CREATE, Body::from(empty.to_string())))
            .await
            .status,
        StatusCode::CREATED
    );
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn task_text_publish_decoded_text_and_derived_task_title_have_independent_utf8_limits() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for title in [
        "".into(),
        " \t".into(),
        "x".repeat(257),
        "😀".repeat(65),
        "bad\nname".into(),
        "bad\u{85}".into(),
    ] {
        let mut changed = wire();
        changed["expected"]["task"]["title"] = json!(title);
        changed["expected"]["manifest"]["title"] = json!(title);
        invalid(app.clone(), changed.to_string()).await;
    }
    for text in [
        "x".repeat(262145),
        "😀".repeat(65537),
        "bad\0text".into(),
        "bad\u{7f}".into(),
        "bad\u{85}".into(),
    ] {
        let mut changed = wire();
        changed["text"] = json!(text);
        invalid(app.clone(), changed.to_string()).await;
    }
    invalid(app.clone(), vec![0xff, 0xfe]).await;
    invalid(
        app.clone(),
        wire().to_string().replace("Private notes", "\\ud800"),
    )
    .await;
    assert_eq!(backend.calls(), 0);
    for (title, text) in [
        ("😀".repeat(64), "".into()),
        ("x".repeat(256), "\t中文😀\r\n".into()),
        ("Notes".into(), "x".repeat(262144)),
        ("Notes".into(), "文".repeat(87381) + "x"),
    ] {
        let mut changed = wire();
        changed["expected"]["task"]["title"] = json!(title);
        changed["expected"]["manifest"]["title"] = json!(title);
        changed["text"] = json!(text);
        let response = capture(
            app.clone(),
            request("POST", CREATE, Body::from(changed.to_string())),
        )
        .await;
        assert_eq!(response.status, StatusCode::CREATED);
        assert_eq!(response.json["artifact"]["title"], json!(title));
        assert_eq!(response.json["artifact"]["byte_length"], text.len());
    }
    assert_eq!(backend.calls(), 4);
}

#[tokio::test]
async fn task_text_publish_wire_budget_counts_actual_stream_bytes_and_accepts_exact_two_mib() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let mut empty = wire();
    empty["text"] = json!("");
    let raw = empty.to_string();
    let escaped = raw.replace(
        "\"text\":\"\"",
        &format!("\"text\":\"{}\"", "\\u0078".repeat(262144)),
    );
    assert!(escaped.len() > 1024 * 1024 && escaped.len() < WIRE_LIMIT);
    assert_eq!(
        capture(app.clone(), request("POST", CREATE, Body::from(escaped)))
            .await
            .status,
        StatusCode::CREATED
    );
    let mut exact = raw.into_bytes();
    exact.resize(WIRE_LIMIT, b' ');
    assert_eq!(
        capture(
            app.clone(),
            request("POST", CREATE, Body::from(exact.clone()))
        )
        .await
        .status,
        StatusCode::CREATED
    );
    assert_eq!(backend.calls(), 2);
    for declared in [None, Some("1")] {
        let stream = futures_util::stream::iter([
            Ok::<_, std::io::Error>(Bytes::from(exact.clone())),
            Ok(Bytes::from_static(b" ")),
        ]);
        let mut req = request("POST", CREATE, Body::from_stream(stream));
        if let Some(value) = declared {
            req.headers_mut()
                .insert(header::CONTENT_LENGTH, value.parse().unwrap());
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 2);
}

#[tokio::test]
async fn task_text_publish_media_encoding_trailers_and_stream_errors_reject_before_dispatch() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for media in [
        None,
        Some("text/plain"),
        Some("application/json;charset=utf-8"),
    ] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut().remove(header::CONTENT_TYPE);
        if let Some(value) = media {
            req.headers_mut()
                .insert(header::CONTENT_TYPE, value.parse().unwrap());
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "not_attempted",
        );
    }
    let mut duplicate = request("POST", CREATE, untouched());
    duplicate.headers_mut().append(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    error(
        &capture(app.clone(), duplicate).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        "not_attempted",
    );
    let mut encoded = request("POST", CREATE, untouched());
    encoded.headers_mut().insert(
        header::CONTENT_ENCODING,
        HeaderValue::from_static("identity"),
    );
    error(
        &capture(app.clone(), encoded).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    let broken = Body::from_stream(futures_util::stream::iter([
        Ok(Bytes::from(wire().to_string())),
        Err(std::io::Error::other("synthetic after complete JSON")),
    ]));
    error(
        &capture(app.clone(), request("POST", CREATE, broken)).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    let frames = futures_util::stream::iter([
        Ok::<_, std::io::Error>(http_body::Frame::data(Bytes::from(wire().to_string()))),
        Ok(http_body::Frame::trailers(HeaderMap::new())),
    ]);
    error(
        &capture(
            app.clone(),
            request(
                "POST",
                CREATE,
                Body::new(http_body_util::StreamBody::new(frames)),
            ),
        )
        .await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
    let mut valid = request("POST", CREATE, body());
    valid.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    assert_eq!(capture(app, valid).await.status, StatusCode::CREATED);
    assert_eq!(backend.calls(), 1);
}
