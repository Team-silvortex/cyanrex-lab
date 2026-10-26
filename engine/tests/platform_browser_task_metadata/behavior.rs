use super::{
    child::{safe_threads, stopped},
    fixture::*,
    process::Browser,
};
use serde_json::json;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL, explicit Playwright/Chromium paths and non-root capless child support"]
async fn postgres_platform_browser_task_metadata_confirmed_replace_conflict_and_revocation() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            exact_profile(f).await;
            let original = p.current().await;
            assert_eq!(u64::from(original.task.revision), 1);
            assert_eq!(original.manifest.payload().len(), 2);
            let immutable = immutable_rows(f).await;
            let files = p.files();
            let initial_counter = counter(f).await;
            let mut host = p.spawn();
            assert!(host.ready().await);
            assert!(safe_threads(host.pid(), &p.identity));
            let mut browser = Browser::start(p.input()).await;
            browser.phase("ready_login").await;
            p.assert_head_event(&original, 1).await;
            assert_eq!(initial_counter, counter(f).await);
            browser
                .send(json!({"otp":super::login_fixture::otp()}))
                .await;
            let digest = browser.phase("authenticated").await["digest"]
                .as_str()
                .unwrap()
                .to_owned();
            exact_session(f, &digest).await;
            let consumed = counter(f).await;
            assert!(consumed > initial_counter);
            let issued_sessions = sessions(f).await;
            assert_eq!(immutable, immutable_rows(f).await);
            assert_eq!(files, p.files());
            browser.advance().await;

            browser.phase("saved").await;
            let saved = p.current().await;
            assert_eq!(u64::from(saved.task.revision), 2);
            assert_eq!(saved.manifest, saved_manifest(&original));
            assert_eq!(saved.task.title, SAVED_TITLE);
            assert_eq!(saved.task.owner, original.task.owner);
            assert_eq!(saved.task.reference, original.task.reference);
            assert_eq!(saved.task.created_at, original.task.created_at);
            assert_eq!(saved.task.status, original.task.status);
            assert_eq!(saved.task.definition, None);
            assert_eq!(saved.task.input_refs, original.task.input_refs);
            p.assert_head_event(&saved, 2).await;
            assert_eq!(immutable, immutable_rows(f).await);
            assert_eq!(files, p.files());
            assert_eq!(issued_sessions, sessions(f).await);
            browser.advance().await;

            browser.phase("conflict_ready").await;
            p.assert_head_event(&saved, 2).await;
            let winner = p.admin_advance(&saved).await;
            assert_eq!(u64::from(winner.task.revision), 3);
            assert_eq!(winner.task.title, ADMIN_TITLE);
            p.assert_head_event(&winner, 3).await;
            assert_eq!(immutable, immutable_rows(f).await);
            assert_eq!(files, p.files());
            browser.advance().await;
            browser.phase("conflict").await;
            // The UI's stale expected_revision=2 cannot overwrite the legal winner or emit an
            // event. Its retained draft is not a committed snapshot and is never retried.
            p.assert_head_event(&winner, 3).await;
            assert_eq!(immutable, immutable_rows(f).await);
            assert_eq!(files, p.files());
            assert_eq!(issued_sessions, sessions(f).await);
            assert_eq!(consumed, counter(f).await);
            browser.advance().await;

            browser.phase("revoked_ready").await;
            p.assert_head_event(&winner, 3).await;
            exact_session(f, &digest).await;
            revoke(f, &digest).await;
            let revoked_sessions = sessions(f).await;
            assert_ne!(issued_sessions, revoked_sessions);
            browser.advance().await;
            browser.phase("denied").await;
            p.assert_head_event(&winner, 3).await;
            assert_eq!(immutable, immutable_rows(f).await);
            assert_eq!(files, p.files());
            assert_eq!(revoked_sessions, sessions(f).await);
            assert_eq!(consumed, counter(f).await);
            exact_profile(f).await;
            browser.advance().await;
            browser.phase("complete").await;
            browser.finish().await;
            p.assert_head_event(&winner, 3).await;
            assert_eq!(immutable, immutable_rows(f).await);
            assert_eq!(files, p.files());
            assert_eq!(revoked_sessions, sessions(f).await);
            assert!(stopped(&host.stop().await));
            assert!(within(tokio::net::TcpStream::connect(p.bind))
                .await
                .is_err());
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
