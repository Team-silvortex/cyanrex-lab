//! Actual binary owners and current Source authority, not synthetic semaphore receipts.
use super::*;
use axum::{body::HttpBody, http::StatusCode};
use cyanrex_engine::{
    platform_http::{build_file_distribution_router, FileDistributionHttpState},
    services::auth_service::durable_source::SessionFileChunkReadError,
};
use distribution_bridge::bounded;
use http_body_util::BodyExt;

fn request(token: &str, path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header("x-cyanrex-platform-request", "1")
        .header("cookie", format!("cyanrex_platform_session={token}"))
        .body(Body::empty())
        .unwrap()
}
fn paths(selected: &SessionFileChunkReadRequest) -> (String, String) {
    let file = &selected.download.expected_file;
    let query = format!(
        "grant_id={}&upload_id={}&file_id={}&version_id={}&byte_length={}&sha256={}",
        selected.download.grant_id,
        selected.download.upload_id,
        file.file_id(),
        file.version_id(),
        file.byte_length(),
        file.sha256()
    );
    (
        format!("/platform/v1/file-manifests?{query}"),
        format!(
            "/platform/v1/file-chunks?{query}&chunk_index={}",
            selected.chunk_index
        ),
    )
}
fn private(response: &axum::response::Response) {
    for (name, value) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
    ] {
        assert_eq!(response.headers()[name], value);
    }
    for name in [
        "set-cookie",
        "location",
        "retry-after",
        "access-control-allow-origin",
        "content-range",
        "etag",
        "x-cyanrex-source-key",
        "x-cyanrex-root-key",
    ] {
        assert!(!response.headers().contains_key(name));
    }
}
async fn error(
    route: &Router,
    token: &str,
    path: &str,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    let response = bounded(route.clone().oneshot(request(token, path)))
        .await
        .unwrap();
    private(&response);
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()["content-type"], "application/json");
    let data = bounded(response.into_body().collect())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&data).unwrap(),
        serde_json::json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
async fn source_full(f: &chunk_fixture::Fixture) {
    assert_eq!(
        bounded(f.sharing.base.grants.auth.source.read_session_file_chunk(
            &f.sharing.base.grants.auth.target_token,
            &f.workspace,
            &f.request(),
        ))
        .await
        .err(),
        Some(SessionFileChunkReadError::CapacityExhausted)
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_distribution_keeps_binary_owners_and_metadata_independent_across_clones_and_revocation(
) {
    checked(chunk_fixture::Fixture::ready(1).await, |f| {
        Box::pin(async move {
            let source = &f.sharing.base.grants.auth.source;
            let token = &f.sharing.base.grants.auth.target_token;
            let state = FileDistributionHttpState::with_capacities(
                source.clone(),
                f.workspace.clone(),
                "https://platform.example",
                1,
                1,
            )
            .unwrap();
            let route = build_file_distribution_router(state.clone());
            let other = build_file_distribution_router(state);
            let selected = f.request();
            let (metadata_path, chunk_path) = paths(&selected);
            let before = f.sharing.snapshot().await;
            let response = bounded(route.clone().oneshot(request(token, &chunk_path)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            private(&response);
            assert_eq!(
                response.headers()["content-type"],
                "application/octet-stream"
            );
            assert_eq!(response.headers()["content-length"], "17");
            assert_eq!(
                response.headers()["x-cyanrex-file-sha256"],
                selected.download.expected_file.sha256().to_string()
            );
            let mut body = response.into_body();
            let frame = bounded(body.frame())
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap();
            assert!(body.is_end_stream());
            assert_eq!(
                frame.as_ref(),
                &(0..17).map(bytes::byte).collect::<Vec<_>>()
            );
            let clone = frame.clone();
            let slice = clone.slice(1..3);
            drop((body, frame));
            error(
                &other,
                token,
                &chunk_path,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            source_full(f).await;
            // The byte-owning response cannot exhaust this separate metadata HTTP gate or Source read.
            let metadata = bounded(other.clone().oneshot(request(token, &metadata_path)))
                .await
                .unwrap();
            assert_eq!(metadata.status(), StatusCode::OK);
            private(&metadata);
            error(
                &route,
                token,
                &metadata_path,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            let mut metadata_body = metadata.into_body();
            let json = bounded(metadata_body.frame())
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap();
            assert!(metadata_body.is_end_stream());
            let manifest: FileTransferManifest = serde_json::from_slice(&json).unwrap();
            assert_eq!(
                FileVersionRef::from_manifest(&manifest),
                selected.download.expected_file
            );
            // Unlike binary owner-retaining Bytes, keeping JSON bytes does not retain its HTTP slot.
            let retained_json = json.clone();
            drop((json, metadata_body));
            let metadata = bounded(route.clone().oneshot(request(token, &metadata_path)))
                .await
                .unwrap();
            assert_eq!(metadata.status(), StatusCode::OK);
            let current = bounded(metadata.into_body().collect())
                .await
                .unwrap()
                .to_bytes();
            assert_eq!(current, retained_json);
            drop(current);
            drop(clone);
            assert_eq!(slice.as_ref(), &[1, 2]);
            error(
                &route,
                token,
                &chunk_path,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            source_full(f).await;
            assert_eq!(f.sharing.snapshot().await, before);
            f.sharing
                .store
                .revoke(
                    sharing_fixture::recipient_grant(1),
                    f.sharing.actor(),
                    sharing_fixture::revision(1),
                )
                .await
                .unwrap();
            let revoked = f.sharing.snapshot().await;
            sharing_fixture::recipient_delta(&f.sharing.schema, &before, &revoked, 1, true);
            error(
                &other,
                token,
                &metadata_path,
                StatusCode::NOT_FOUND,
                "not_found",
                "unconfirmed",
            )
            .await;
            assert_eq!(
                slice.as_ref(),
                &[1, 2],
                "later revocation cannot reclaim an already-approved byte snapshot"
            );
            source_full(f).await;
            drop(slice);
            error(
                &route,
                token,
                &chunk_path,
                StatusCode::NOT_FOUND,
                "not_found",
                "unconfirmed",
            )
            .await;
            error(
                &other,
                token,
                &metadata_path,
                StatusCode::NOT_FOUND,
                "not_found",
                "unconfirmed",
            )
            .await;
            assert_eq!(f.sharing.snapshot().await, revoked);
            drop(retained_json);
            f.sharing.base.grants.auth.base.drain().await;
        })
    })
    .await;
}
