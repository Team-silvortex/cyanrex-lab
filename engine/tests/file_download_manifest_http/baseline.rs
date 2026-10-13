use crate::{fixture::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_manifest_http_returns_complete_private_published_manifest_for_current_exact_recipient(
) {
    let (
        status,
        headers,
        returned,
        expected,
        request,
        recipient,
        policy,
        issued,
        before,
        after,
        original_files,
        final_files,
    ) = checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let typed = f.request();
            let expected = f.sharing.base.manifest.clone();
            let recipient = f.sharing.recipient;
            let auth = &f.sharing.base.grants.auth;
            let policy = auth.current(recipient.principal().principal_id).await;
            let issued = f.issued.clone();
            let before = f.sharing.snapshot().await;
            let original_files = bytes::tree(&f.sharing.base.directory);
            let response = app(f)
                .oneshot(request(&auth.target_token, &path(&typed)))
                .await
                .unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let returned = response
                .into_body()
                .collect()
                .await
                .map(|body| body.to_bytes().to_vec())
                .map_err(|error| error.to_string());
            let after = f.sharing.snapshot().await;
            let final_files = bytes::tree(&f.sharing.base.directory);
            (
                status,
                headers,
                returned,
                expected,
                typed,
                recipient,
                policy,
                issued,
                before,
                after,
                original_files,
                final_files,
            )
        })
    })
    .await;

    assert_eq!(status, StatusCode::OK,
        "real current exact recipient must receive the full trusted Published manifest, not placeholder metadata");
    let returned = returned.expect("private JSON body must complete normally");
    assert!(returned.len() <= MAX_FILE_TRANSFER_MANIFEST_BYTES);
    assert_eq!(headers["content-type"], "application/json");
    assert_eq!(headers["content-length"], returned.len().to_string());
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(headers["pragma"], "no-cache");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    for name in [
        "set-cookie",
        "location",
        "retry-after",
        "access-control-allow-origin",
        "content-disposition",
        "content-range",
        "etag",
        "last-modified",
        "x-cyanrex-owner-id",
        "x-cyanrex-account-id",
        "x-cyanrex-source-key",
        "x-cyanrex-root-key",
        "x-cyanrex-journal-key",
    ] {
        assert!(
            !headers.contains_key(name),
            "private manifest must not expose {name}"
        );
    }
    let value: serde_json::Value = serde_json::from_slice(&returned).unwrap();
    assert_eq!(value, serde_json::to_value(&expected).unwrap(),
        "return the direct complete manifest, including ordered chunk digests and inert display metadata");
    assert_eq!(returned, serde_json::to_vec(&expected).unwrap());
    let parsed = FileTransferManifest::parse_json(&returned)
        .expect("shared strict manifest parser accepts actual JSON");
    assert_eq!(parsed, expected);
    assert_eq!(parsed.workspace(), scope());
    assert_eq!(
        FileVersionRef::from_manifest(&parsed),
        request.expected_file
    );
    assert_eq!(parsed.byte_length(), 17);
    assert_eq!(parsed.chunk_sha256().len(), 1);
    assert_eq!(parsed.chunk(0).unwrap().sha256, *parsed.sha256());
    assert_eq!(parsed.display_name(), "真实作业.bin");
    assert!(policy.policy.membership.role_refs.is_empty());
    assert!(!policy.policy.deployment_granted);
    assert_eq!(policy.policy.membership.status, MembershipStatus::Active);
    assert_eq!(issued.grant().state(), FileGrantState::Active);
    assert_eq!(issued.grant().recipient(), recipient);
    assert_eq!(issued.grant().file(), &request.expected_file);
    assert_eq!(after, before,
        "HTTP manifest observation must not change Auth, Registry, grants, upload, journal, Published rows/OIDs or staged bytes/inodes");
    assert_eq!(
        final_files, original_files,
        "original input file and staging directory remain unchanged"
    );
}
