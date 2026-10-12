use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publisher_grants_filter_exact_subject_and_account_generation() {
    let f = ready(16).await;
    let first = issue(&f, 1, account(1), account(2), validity())
        .await
        .unwrap();
    assert_eq!(first.issuer(), account(1));
    assert_eq!(first.grant().subject(), account(2));
    assert_eq!(first.grant().workspace(), scope());
    assert_eq!(first.grant().id(), grant(1));
    assert_eq!(first.grant().revision(), revision(1));
    assert_eq!(first.grant().state(), FileGrantState::Active);
    assert_eq!(first.grant().validity(), validity());
    assert!(first.grant().matches(scope(), account(2), 150));
    assert!(!first.grant().matches(scope(), account(2), 200));
    assert_eq!(get(&f, 1, account(2)).await.unwrap(), Some(first.clone()));
    for other in [
        account(1),
        account(3),
        FileAccountRef::new(account(2).principal(), account(3).account_id()),
        FileAccountRef::new(account(3).principal(), account(2).account_id()),
    ] {
        assert_eq!(
            get(&f, 1, other).await.unwrap(),
            None,
            "issuer/principal/generation mismatches cannot read the selected subject's metadata"
        );
    }
    assert_eq!(get(&f, 99, account(2)).await.unwrap(), None);
    assert_eq!(
        issue(&f, 1, account(1), account(2), validity()).await,
        Err(FilePublisherGrantStoreError::Conflict)
    );
    assert_eq!(
        issue(
            &f,
            1,
            account(3),
            account(3),
            FileGrantValidity::new(200, 300).unwrap()
        )
        .await,
        Err(FilePublisherGrantStoreError::Conflict)
    );
    let next_generation = FileAccountRef::new(account(2).principal(), account(3).account_id());
    let second = issue(&f, 2, account(1), next_generation, validity())
        .await
        .unwrap();
    assert_eq!(get(&f, 2, account(2)).await.unwrap(), None);
    assert_eq!(get(&f, 2, next_generation).await.unwrap(), Some(second));
    // Independent IDs may carry identical subject/issuer/window metadata; there is no implicit
    // one-grant-per-subject policy or Registry role lookup in this trusted direct store.
    issue(&f, 3, account(1), account(2), validity())
        .await
        .unwrap();
    let (reopened, pool) = f.reopen().await;
    let before = snapshot(&f).await;
    let after_reopen = reopened.get(grant(1), account(2)).await;
    let after = snapshot(&f).await;
    pool.close().await;
    restored(&f).await;
    f.cleanup().await;
    assert_eq!(after_reopen.unwrap(), Some(first));
    assert_eq!(after, before);
}
