use super::fixture::*;
use axum::{
    body::{Body, Bytes},
    http::{HeaderValue, StatusCode},
};
use cyanrex_engine::models::collaboration::MAX_TEXT_PAYLOAD_BYTES;
use serde_json::{json, Value};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_http_strict_dto_path_and_ingress_reject_before_io() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let old = p
                .task_manifest(
                    10,
                    0,
                    manifest(
                        "Exact expected metadata",
                        vec![p.reference(777, b"Old pin metadata only")],
                    ),
                )
                .await;
            let route = router(&p, 1).await;
            let target = path(&old);
            let before = snapshot(f).await;
            let files = p.files();
            assert!(p.root.swap());
            for method in ["GET", "PUT", "PATCH", "DELETE", "OPTIONS"] {
                error(
                    &route,
                    request(&p, 0, method, &target, unread()),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "method_not_allowed",
                    "not_attempted",
                )
                .await;
            }
            for absent in [
                format!("{target}/"),
                format!("{target}/extra"),
                "/platform/v1/text-artifacts".into(),
                "/platform/v1/tasks".into(),
            ] {
                error(
                    &route,
                    request(&p, 0, "POST", &absent, unread()),
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "not_attempted",
                )
                .await;
            }
            for invalid in [
                format!("{target}?owner=other"),
                target
                    .to_uppercase()
                    .replace("/PLATFORM/V1/TASKS/", "/platform/v1/tasks/")
                    .replace("/TEXT-ARTIFACTS", "/text-artifacts"),
                "/platform/v1/tasks/00000000-0000-0000-0000-000000000000/text-artifacts".into(),
                "/platform/v1/tasks/not-a-task/text-artifacts".into(),
                target.replacen("/tasks/0", "/tasks/%30", 1),
                format!(
                    "/platform/v1/tasks/{}/text-artifacts",
                    old.task.reference.task_id.to_string().replace('-', "")
                ),
            ] {
                error(
                    &route,
                    request(&p, 0, "POST", &invalid, unread()),
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
            }
            for key in ["origin", "x-cyanrex-platform-request"] {
                for mode in ["missing", "duplicate", "wrong"] {
                    let mut req = request(&p, 0, "POST", &target, unread());
                    if mode == "missing" {
                        req.headers_mut().remove(key);
                    } else if mode == "duplicate" {
                        let value = req.headers()[key].clone();
                        req.headers_mut().append(key, value);
                    } else {
                        req.headers_mut()
                            .insert(key, HeaderValue::from_static("wrong"));
                    }
                    error(
                        &route,
                        req,
                        StatusCode::FORBIDDEN,
                        "request_forbidden",
                        "not_attempted",
                    )
                    .await;
                }
            }
            for cookie in [
                String::new(),
                "session=legacy".into(),
                "cyanrex_platform_session=bad".into(),
                format!(
                    "cyanrex_platform_session={0}; cyanrex_platform_session={0}",
                    p.tokens[0]
                ),
            ] {
                let mut req = request(&p, 0, "POST", &target, unread());
                req.headers_mut()
                    .insert("cookie", HeaderValue::from_str(&cookie).unwrap());
                error(
                    &route,
                    req,
                    StatusCode::UNAUTHORIZED,
                    "invalid_session",
                    "not_attempted",
                )
                .await;
            }
            let mut req = request(&p, 0, "POST", &target, unread());
            req.headers_mut().insert(
                "authorization",
                HeaderValue::from_static("Bearer not-a-session"),
            );
            error(
                &route,
                req,
                StatusCode::UNAUTHORIZED,
                "invalid_session",
                "not_attempted",
            )
            .await;
            for (key, value, status, code) in [
                (
                    "content-type",
                    "text/plain",
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "unsupported_media_type",
                ),
                (
                    "content-encoding",
                    "gzip",
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                ),
            ] {
                let mut req = request(&p, 0, "POST", &target, unread());
                req.headers_mut()
                    .insert(key, HeaderValue::from_static(value));
                error(&route, req, status, code, "not_attempted").await;
            }
            let valid = payload(&old, 20, "Valid text");
            let raw = valid.to_string();
            let mut invalid: Vec<String> = vec![
                "null".into(),
                "[]".into(),
                "{}".into(),
                format!("\u{feff}{raw}"),
                format!("{raw} {{}}"),
            ];
            for key in [
                "schema_version",
                "expected",
                "artifact_id",
                "revision_id",
                "text",
            ] {
                invalid.push(raw.replacen(
                    &format!("\"{key}\":"),
                    &format!("\"{key}\":null,\"{key}\":"),
                    1,
                ));
                let mut value = valid.clone();
                value.as_object_mut().unwrap().remove(key);
                invalid.push(value.to_string());
            }
            for key in [
                "owner",
                "scope",
                "namespace",
                "path",
                "title",
                "filename",
                "sha256",
                "kind",
                "media_type",
                "expected_revision",
                "contents",
            ] {
                let mut value = valid.clone();
                value[key] = json!("not caller authority");
                invalid.push(value.to_string());
            }
            for pointer in [
                "/expected",
                "/expected/task",
                "/expected/manifest",
                "/expected/task/reference",
                "/expected/task/owner",
                "/expected/task/reference/workspace",
                "/expected/task/input_refs/0",
                "/expected/task/input_refs/0/workspace",
                "/expected/manifest/payload/0",
                "/expected/manifest/payload/0/artifact",
                "/expected/manifest/payload/0/artifact/workspace",
            ] {
                let mut value = valid.clone();
                value
                    .pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("unknown".into(), Value::Null);
                invalid.push(value.to_string());
                let mut value = valid.clone();
                *value.pointer_mut(pointer).unwrap() = Value::Array(vec![]);
                invalid.push(value.to_string());
            }
            for key in ["task", "manifest"] {
                let mut value = valid.clone();
                value["expected"].as_object_mut().unwrap().remove(key);
                invalid.push(value.to_string());
            }
            let mut value = valid.clone();
            value["expected"]["task"]
                .as_object_mut()
                .unwrap()
                .remove("definition");
            invalid.push(value.to_string());
            let mut value = valid.clone();
            value["expected"]["task"]["definition"] = json!({});
            invalid.push(value.to_string());
            let task_raw = valid["expected"]["task"].to_string();
            let duplicated = task_raw.replacen("\"title\":", "\"title\":null,\"title\":", 1);
            invalid.push(raw.replacen(&task_raw, &duplicated, 1));
            for (pointer, field) in [
                ("/expected/task/reference", "task_id"),
                ("/expected/task/owner", "principal_id"),
                ("/expected/task/reference/workspace", "workspace_id"),
                ("/expected/task/input_refs/0", "artifact_id"),
                ("/expected/manifest", "title"),
                ("/expected/manifest/payload/0", "filename"),
                ("/expected/manifest/payload/0/artifact", "sha256"),
            ] {
                let record = valid.pointer(pointer).unwrap().to_string();
                let duplicated = record.replacen(
                    &format!("\"{field}\":"),
                    &format!("\"{field}\":null,\"{field}\":"),
                    1,
                );
                invalid.push(raw.replacen(&record, &duplicated, 1));
            }
            let expected_raw = valid["expected"].to_string();
            invalid.push(raw.replacen(
                &expected_raw,
                &expected_raw.replacen("\"task\":", "\"task\":null,\"task\":", 1),
                1,
            ));
            for (pointer, bad) in [
                ("/schema_version", json!(2)),
                ("/schema_version", json!(1.0)),
                ("/artifact_id", json!(uuid::Uuid::nil())),
                ("/revision_id", json!("not-an-id")),
                ("/expected/task/revision", json!(0)),
                ("/expected/task/revision", json!(1.0)),
                ("/expected/task/title", json!("Different from manifest")),
                ("/text", json!("x".repeat(MAX_TEXT_PAYLOAD_BYTES + 1))),
                ("/text", json!("NUL\0")),
                ("/text", json!("DEL\u{7f}")),
                ("/text", json!("ESC\u{1b}")),
            ] {
                let mut value = valid.clone();
                *value.pointer_mut(pointer).unwrap() = bad;
                invalid.push(value.to_string());
            }
            let mut foreign = old.clone();
            foreign.task.reference.task_id = task_id(11);
            invalid.push(payload(&foreign, 20, "Path mismatch").to_string());
            let mut foreign = old.clone();
            foreign.task.reference.workspace.workspace_id =
                uuid::Uuid::new_v4().try_into().unwrap();
            invalid.push(payload(&foreign, 20, "Wrong configured scope").to_string());
            for raw in invalid {
                error(
                    &route,
                    request(&p, 0, "POST", &target, Body::from(raw)),
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
            }
            error(
                &route,
                request(&p, 0, "POST", &target, Body::from(vec![0xff, 0xfe])),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "not_attempted",
            )
            .await;
            let exact = format!("{raw}{}", " ".repeat(2 * 1024 * 1024 - raw.len()));
            let stream = futures_util::stream::iter([
                Ok::<_, std::io::Error>(Bytes::from(exact)),
                Ok(Bytes::from_static(b" ")),
            ]);
            let mut req = request(&p, 0, "POST", &target, Body::from_stream(stream));
            req.headers_mut()
                .insert("content-length", HeaderValue::from_static("1"));
            error(
                &route,
                req,
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_too_large",
                "not_attempted",
            )
            .await;
            assert!(p.root.restore());
            assert_eq!(before, snapshot(f).await);
            assert_eq!(files, p.files());
            p.counts(0).await;
            drop(route);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
