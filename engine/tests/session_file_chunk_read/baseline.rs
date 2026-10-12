use crate::{fixture::Fixture, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_current_recipient_reads_verified_original_chunk_and_retains_capacity_until_drop()
{
    let f = Fixture::ready(1).await;
    let auth = &f.sharing.base.grants.auth;
    let policy = auth
        .current(f.sharing.recipient.principal().principal_id)
        .await;
    let request = f.request();
    let expected_file = f.sharing.expected_file.clone();
    let expected_bytes = (0..17).map(bytes::byte).collect::<Vec<_>>();
    let before = f.sharing.snapshot().await;
    let first = auth
        .source
        .read_session_file_chunk(&auth.target_token, &f.workspace, &request)
        .await;
    // A test-owned copy is the oracle comparison only; the service exposes no intoVec/Clone.
    let first_file = first.as_ref().ok().map(|r| r.file().clone());
    let first_index = first.as_ref().ok().map(|r| r.chunk_index());
    let first_bytes = first.as_ref().ok().map(|r| r.bytes().to_vec());
    let first_error = first.as_ref().err().copied();
    let after_first = f.sharing.snapshot().await;
    let rejected = auth
        .source
        .read_session_file_chunk(&auth.target_token, &f.workspace, &request)
        .await;
    let rejection_error = rejected.err();
    let after_rejected = f.sharing.snapshot().await;
    drop(first); // The returned immutable byte result, not just its caller future, owns the lease.
    let retry = auth
        .source
        .read_session_file_chunk(&auth.target_token, &f.workspace, &request)
        .await;
    let retry_file = retry.as_ref().ok().map(|r| r.file().clone());
    let retry_index = retry.as_ref().ok().map(|r| r.chunk_index());
    let retry_bytes = retry.as_ref().ok().map(|r| r.bytes().to_vec());
    let retry_error = retry.as_ref().err().copied();
    drop(retry);
    let after_retry = f.sharing.snapshot().await;
    f.cleanup().await;

    assert!(policy.policy.membership.role_refs.is_empty());
    assert!(!policy.policy.deployment_granted);
    assert_eq!(policy.policy.membership.status, MembershipStatus::Active);
    assert_eq!(
        first_error, None,
        "current exact recipient must obtain actual verified inert chunk bytes"
    );
    assert_eq!(first_file, Some(expected_file.clone()));
    assert_eq!(first_index, Some(0));
    assert_eq!(first_bytes, Some(expected_bytes.clone()));
    assert_eq!(
        after_first, before,
        "byte observation may not mutate metadata/OIDs or staged file bytes/identity"
    );
    assert_eq!(
        rejection_error,
        Some(SessionFileChunkReadError::CapacityExhausted),
        "the first returned result must retain the sole byte-buffer lease"
    );
    assert_eq!(after_rejected, before);
    assert_eq!(
        retry_error, None,
        "dropping the original result must restore bounded byte admission"
    );
    assert_eq!(retry_file, Some(expected_file));
    assert_eq!(retry_index, Some(0));
    assert_eq!(retry_bytes, Some(expected_bytes));
    assert_eq!(after_retry, before);
}
