use crate::{fixture::Fixture, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_current_recipient_observes_exact_published_manifest_after_publisher_revocation() {
    let f = Fixture::ready().await;
    let auth = &f.sharing.base.grants.auth;
    let recipient = f.sharing.recipient;
    let policy = auth.current(recipient.principal().principal_id).await;
    let request = f.request();
    let expected = f.sharing.base.manifest.clone();
    let issued = f.issued.clone();
    let before = f.sharing.snapshot().await;
    let first = auth
        .source
        .get_session_file_download_manifest(&auth.target_token, &f.workspace, &request)
        .await;
    let after_first = f.sharing.snapshot().await;
    let publisher_revoked = f
        .sharing
        .base
        .grants
        .store
        .revoke(
            grant_fixture::grant(1),
            f.sharing.base.grants.issuer(),
            sharing_fixture::revision(1),
        )
        .await;
    let after_publisher_revoked = f.sharing.snapshot().await;
    let second = auth
        .source
        .get_session_file_download_manifest(&auth.target_token, &f.workspace, &request)
        .await;
    let after_second = f.sharing.snapshot().await;
    let recipient_current = f.sharing.store.get(request.grant_id, recipient).await;
    let after_get = f.sharing.snapshot().await;
    f.cleanup().await;

    assert!(policy.policy.membership.role_refs.is_empty());
    assert!(!policy.policy.deployment_granted);
    assert_eq!(policy.policy.membership.status, MembershipStatus::Active);
    assert_eq!(issued.grant().recipient(), recipient);
    assert_eq!(issued.grant().file(), &request.expected_file);
    assert_eq!(issued.grant().state(), FileGrantState::Active);
    assert_eq!(expected.byte_length(), 17);
    assert_eq!(
        FileVersionRef::from_manifest(&expected),
        request.expected_file
    );
    assert_eq!(
        first.expect("current exact recipient must observe the trusted Published manifest"),
        expected
    );
    assert_eq!(after_first, before, "manifest observation may not mutate any source, Registry, grant, upload, journal, Published row/OID or staged file");
    let revoked = publisher_revoked.expect("fixture publisher revocation must commit");
    assert_eq!(revoked.grant().state(), FileGrantState::Revoked);
    sharing_fixture::publisher_revocation_delta(&after_first, &after_publisher_revoked);
    assert_eq!(
        second.expect(
            "revoking publisher permission does not revoke an independent active recipient grant"
        ),
        expected
    );
    assert_eq!(after_second, after_publisher_revoked);
    assert_eq!(recipient_current.unwrap(), Some(issued));
    assert_eq!(after_get, after_second);
}
