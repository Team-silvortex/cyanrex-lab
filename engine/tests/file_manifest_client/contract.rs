use crate::{fixture::*, support::*, *};
use axum::{
    body::Bytes,
    http::{HeaderName, StatusCode},
};
use serde_json::json;

#[tokio::test]
async fn manifest_json_rejects_duplicates_unknowns_bad_geometry_utf8_and_trailing_values_without_file_work(
) {
    checked(
        Fixture::with_manifest(bytes::manifest(17, 0xab)).await,
        |f| {
            Box::pin(async move {
                let client = f.client();
                let wire = serde_json::to_string(&f.manifest).unwrap();
                let mut bad = vec![
                    format!("{{\"file_id\":\"{}\",{}", f.manifest.file_id(), &wire[1..])
                        .into_bytes(),
                    wire.replacen(
                        "\"workspace\":{",
                        &format!(
                            "\"workspace\":{{\"authority_id\":\"{}\",",
                            scope().authority_id
                        ),
                        1,
                    )
                    .into_bytes(),
                    format!("{wire}{{}}").into_bytes(),
                    format!("[{wire}]").into_bytes(),
                    b"{\xff}".to_vec(),
                ];
                // Each mutation remains separate so strict parsing, not merely tuple comparison, rejects it.
                for (field, mutation) in [
                    ("schema_version", json!(2)),
                    ("unexpected_receipt", json!(true)),
                    ("chunk_size", json!(1)),
                    ("chunk_sha256", json!([])),
                    ("byte_length", json!(MAX_FILE_TRANSFER_BYTES + 1)),
                    ("display_name", json!("../not-a-path")),
                    ("sha256", json!("A".repeat(64))),
                    ("file_id", json!("00000000-0000-0000-0000-000000000000")),
                ] {
                    let mut value = serde_json::to_value(&f.manifest).unwrap();
                    value[field] = mutation;
                    bad.push(serde_json::to_vec(&value).unwrap());
                }
                let count = bad.len();
                for data in bad {
                    let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                    reply.json_bytes(data);
                    f.set_reply(Kind::Manifest, reply);
                    denied(f, &client, FileManifestClientError::InvalidResponse).await;
                }
                assert_eq!(f.request_count(), count);
                healthy(f, &client).await;
                let requests = f.take_requests();
                assert_eq!(requests.len(), count + 1);
                for request in requests {
                    assert_request(&request, &f.manifest, None);
                }
                assert!(bytes::tree(&f.root).is_empty());
            })
        },
    )
    .await;
}

#[tokio::test]
async fn manifest_matches_all_six_version_components_but_display_and_chunk_metadata_remain_claims()
{
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let client = f.client();
            let mut other_authority = scope();
            other_authority.authority_id = Uuid::from_u128(0xaa).try_into().unwrap();
            let mut other_workspace = scope();
            other_workspace.workspace_id = Uuid::from_u128(0xbb).try_into().unwrap();
            let variants = [
                changed(&f.manifest, "workspace", json!(other_authority)),
                changed(&f.manifest, "workspace", json!(other_workspace)),
                changed(&f.manifest, "file_id", json!(Uuid::from_u128(0xcc))),
                changed(&f.manifest, "version_id", json!(Uuid::from_u128(0xdd))),
                changed(
                    &f.manifest,
                    "byte_length",
                    json!(f.manifest.byte_length() + 1),
                ),
                changed(&f.manifest, "sha256", json!("b".repeat(64))),
            ];
            for remote in variants {
                assert_ne!(
                    FileVersionRef::from_manifest(&remote),
                    FileVersionRef::from_manifest(&f.manifest)
                );
                let encoded = serde_json::to_vec(&remote).unwrap();
                assert_eq!(FileTransferManifest::parse_json(&encoded).unwrap(), remote);
                f.set_reply(Kind::Manifest, Reply::good(&remote, Kind::Manifest));
                denied(f, &client, FileManifestClientError::InvalidResponse).await;
            }
            let mut digests = f.manifest.chunk_sha256().to_vec();
            digests.reverse();
            let claimed = changed(
                &changed(&f.manifest, "display_name", json!("inert.bin")),
                "chunk_sha256",
                json!(digests),
            );
            f.set_reply(Kind::Manifest, Reply::good(&claimed, Kind::Manifest));
            assert_eq!(metadata(f, &client).await.unwrap(), claimed);
            assert_eq!(f.request_count(), 7);
            assert!(
                bytes::tree(&f.root).is_empty(),
                "metadata equality is not verification of stored or remote bytes"
            );
            healthy(f, &client).await;
        })
    })
    .await;
}

#[tokio::test]
async fn manifest_response_requires_single_private_headers_and_rejects_forbidden_fields_and_header_budgets(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let client = f.client();
            let mut replies = Vec::new();
            for name in [
                "content-type",
                "content-length",
                "cache-control",
                "pragma",
                "x-content-type-options",
            ] {
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                reply.headers.remove(name);
                replies.push(reply);
            }
            for (name, value) in [
                ("content-type", "application/json; charset=utf-8"),
                ("cache-control", "private"),
                ("pragma", "cache"),
                ("x-content-type-options", "NoSniff"),
            ] {
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                reply.headers.insert(name, value.parse().unwrap());
                replies.push(reply);
            }
            for name in [
                "content-type",
                "cache-control",
                "pragma",
                "x-content-type-options",
            ] {
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                let value = reply.headers[name].clone();
                reply.headers.append(name, value);
                replies.push(reply);
            }
            for name in [
                "content-encoding",
                "content-range",
                "trailer",
                "set-cookie",
                "location",
                "etag",
                "last-modified",
                "retry-after",
                "content-disposition",
                "x-cyanrex-file-transfer-version",
                "x-cyanrex-file-id",
                "x-cyanrex-file-version-id",
                "x-cyanrex-chunk-index",
                "x-cyanrex-chunk-sha256",
                "x-cyanrex-file-sha256",
                "access-control-allow-origin",
                "access-control-unknown-private-extension",
            ] {
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                reply.headers.insert(name, "forbidden".parse().unwrap());
                replies.push(reply);
            }
            let mut count = Reply::good(&f.manifest, Kind::Manifest);
            for index in 0..65 {
                count.headers.insert(
                    HeaderName::from_bytes(format!("x-controlled-{index}").as_bytes()).unwrap(),
                    "1".parse().unwrap(),
                );
            }
            replies.push(count);
            let mut size = Reply::good(&f.manifest, Kind::Manifest);
            size.headers
                .insert("x-controlled-padding", "p".repeat(8192).parse().unwrap());
            replies.push(size);
            let count = replies.len();
            for reply in replies {
                f.set_reply(Kind::Manifest, reply);
                denied(f, &client, FileManifestClientError::InvalidResponse).await;
            }
            assert_eq!(f.request_count(), count);
            healthy(f, &client).await;
            for request in f.take_requests() {
                assert_request(&request, &f.manifest, None);
            }
        })
    })
    .await;
}

#[tokio::test]
async fn manifest_uses_actual_framed_bytes_and_accepts_maximum_bounded_json_not_only_length_hints()
{
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let client = f.client();
            let mut invalid = Vec::new();
            let mut zero = Reply::good(&f.manifest, Kind::Manifest);
            zero.json_bytes(Bytes::new());
            invalid.push(zero);
            for length in [
                format!("0{}", serde_json::to_vec(&f.manifest).unwrap().len()),
                (MAX_FILE_TRANSFER_MANIFEST_BYTES + 1).to_string(),
            ] {
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                reply
                    .headers
                    .insert("content-length", length.parse().unwrap());
                if length == (MAX_FILE_TRANSFER_MANIFEST_BYTES + 1).to_string() {
                    // Keep the native writer from detecting a short body before flushing headers.
                    // Fixture cleanup owns and releases this gate even when the assertion fails.
                    reply.pieces[0].gate =
                        Some(std::sync::Arc::new(tokio::sync::Semaphore::new(0)));
                }
                invalid.push(reply);
            }
            for reply in invalid {
                f.set_reply(Kind::Manifest, reply);
                denied(f, &client, FileManifestClientError::InvalidResponse).await;
            }
            let mut short = Reply::good(&f.manifest, Kind::Manifest);
            let data = short.pieces[0].data.clone();
            short.pieces = vec![Piece::now(data.slice(..data.len() / 2)), Piece::error()];
            f.set_reply(Kind::Manifest, short);
            let error = metadata(f, &client).await.err().unwrap();
            assert!(
                matches!(
                    error,
                    FileManifestClientError::Unavailable | FileManifestClientError::InvalidResponse
                ),
                "native HTTP can reject truncated framing before the JSON collector"
            );
            redacted(f, &error);
            let maximum = FileTransferManifest::new(
                scope(),
                f.manifest.file_id(),
                f.manifest.version_id(),
                "maximum.bin",
                MAX_FILE_TRANSFER_BYTES,
                vec![f.manifest.sha256().clone(); MAX_FILE_TRANSFER_CHUNKS],
                f.manifest.sha256().clone(),
            )
            .unwrap();
            let mut data = serde_json::to_vec(&serde_json::to_value(&maximum).unwrap()).unwrap();
            assert!(data.len() < MAX_FILE_TRANSFER_MANIFEST_BYTES);
            data.resize(MAX_FILE_TRANSFER_MANIFEST_BYTES, b' ');
            let mut reply = Reply::good(&maximum, Kind::Manifest);
            reply.json_bytes(data);
            f.set_reply(Kind::Manifest, reply);
            let result = within(client.fetch_manifest(
                TOKEN,
                grant(),
                upload(),
                &FileVersionRef::from_manifest(&maximum),
            ))
            .await
            .unwrap();
            assert_eq!(result, maximum);
            assert_eq!(result.chunk_sha256().len(), 16_384);
            assert_eq!(result.byte_length(), 64 * 1024 * 1024 * 1024);
            assert_eq!(f.request_count(), 5);
            assert_request(&f.take_requests().pop().unwrap(), &maximum, None);
            assert!(
                bytes::tree(&f.root).is_empty(),
                "a maximum metadata claim allocates no file or chunk"
            );
            healthy(f, &client).await;
        })
    })
    .await;
}

#[tokio::test]
async fn manifest_non_success_and_redirects_never_retry_or_contact_the_live_redirect_target() {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let client = f.client();
            let statuses = [
                204, 304, 400, 401, 403, 404, 409, 429, 500, 503, 301, 302, 303, 307, 308,
            ];
            for status in statuses {
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                reply.status = StatusCode::from_u16(status).unwrap();
                reply.json_bytes(format!("sensitive error {TOKEN} {}", f.origin));
                if reply.status.is_redirection() {
                    reply.headers.insert(
                        "location",
                        format!("{}/controlled-redirect-target", f.origin)
                            .parse()
                            .unwrap(),
                    );
                }
                f.set_reply(Kind::Manifest, reply);
                denied(f, &client, FileManifestClientError::InvalidResponse).await;
            }
            let requests = f.take_requests();
            assert_eq!(
                requests.len(),
                statuses.len(),
                "one actual request per status and zero redirect target hits"
            );
            for request in requests {
                assert_request(&request, &f.manifest, None);
            }
            assert!(bytes::tree(&f.root).is_empty());
            healthy(f, &client).await;
        })
    })
    .await;
}
