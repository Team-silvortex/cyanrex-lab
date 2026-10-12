use super::{fixture::*, pool_gate, support::*, *};
use std::time::Duration;

/// The first original-upload COMMIT occurs only AFTER the initial Session/grant transaction
/// has committed. Its sole connection return is held until a legitimate grant writer completes.
/// A SHARE fence spanning the file-work gap would deadlock this control instead of authorizing it.
pub async fn original_current_grant_is_not_recaptured_after_the_file_gap() {
    let f = Fixture::ready(17).await;
    f.grants.issue(1, f.actor()).await;
    let (publisher, pool, release, started) = pool_gate::publisher(&f).await;
    let workspace = f
        .grants
        .auth
        .source
        .file_publishing_workspace(publisher.clone(), f.grants.workspace.clone())
        .unwrap();
    let mut a = publisher
        .prepare(upload(1), f.actor().principal(), f.actor().account_id())
        .await
        .unwrap();
    release.arm_count(&pool, 1).await;
    let mut pending = Box::pin(publish_as(
        &f,
        &f.grants.auth.learner_token,
        &workspace,
        1,
        &mut a,
    ));
    tokio::select! {r=&mut pending=>panic!("initial current Session must precede file-work gap: {r:?}"),r=tokio::time::timeout(Duration::from_secs(4),started)=>{r.unwrap().unwrap();}}
    let revoked = tokio::time::timeout(
        Duration::from_secs(3),
        f.grants.store.revoke(
            grant_fixture::grant(1),
            f.grants.issuer(),
            1u64.try_into().unwrap(),
        ),
    )
    .await
    .expect("no source/grant SQL fence may span file work")
    .unwrap();
    let injected = f.snapshot().await;
    drop(release);
    let result = pending.as_mut().await;
    drop(pending);
    let state = a.state();
    let after = f.snapshot().await;
    f.grants.issue(2, f.actor()).await;
    let mut fresh = prepare(&f, 1).await;
    let healthy_result = publish_as(
        &f,
        &f.grants.auth.learner_token,
        &f.workspace,
        2,
        &mut fresh,
    )
    .await;
    let retained = f.grants.store.get(grant_fixture::grant(1), f.actor()).await;
    pool.close().await;
    f.cleanup().await;
    assert_eq!(revoked.grant().state(), FileGrantState::Revoked);
    assert_eq!(
        result.err(),
        Some(grant_error(FilePublisherGrantStoreError::InvalidRecord))
    );
    assert_eq!(state, FileUploadPublicationState::Unknown);
    assert_eq!(after,injected,"independently committed revoked history survives but the pending publication inserts nothing");
    assert!(healthy_result.is_ok());
    assert_eq!(retained.unwrap(), Some(revoked));
}
