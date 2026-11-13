use super::{
    child::{safe_threads, stopped},
    fixture::*,
    process::Browser,
    storage,
};
use serde_json::json;
use std::path::PathBuf;

#[tokio::test]
#[ignore = "requires explicit disposable SQL, Playwright/Chromium and non-root capless children"]
async fn postgres_platform_browser_task_body_plain_confirmed_replace_conflict_and_revocation() {
    exercise(false).await;
}

#[tokio::test]
#[ignore = "requires explicit disposable SQL, browser, OpenSSL/certutil and private XDG NSS without legacy user NSS"]
async fn postgres_platform_browser_task_body_tls_confirmed_replace_conflict_and_revocation() {
    exercise(true).await;
}

async fn exercise(tls: bool) {
    if tls {
        // This refusal precedes creating SQL or files. Never alter HOME/user/system trust or
        // run a plain replacement when the independently selected TLS case cannot be tested.
        let home = PathBuf::from(std::env::var_os("HOME").expect("account directory required"));
        assert!(!home.join(".pki/nssdb").exists(), "TLS browser blocked: legacy user NSS overrides private XDG trust; do not change HOME or bypass verification");
    }
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f, tls).await;
            storage::exact_profile(f).await;
            let catalog = f.snapshot().await;
            let original = p.current().await;
            assert_eq!(u64::from(original.task.revision), 1);
            assert_eq!(original.manifest.payload().len(), 2);
            let immutable = storage::immutable_rows(f).await;
            let original_artifacts = storage::artifact_rows(f).await;
            let files = p.files();
            let initial_counter = counter(f).await;
            let mut host = p.spawn();
            assert!(host.ready().await);
            assert!(safe_threads(host.pid(), &p.identity));
            let mut browser = Browser::start(p.input()).await;
            if tls {
                browser.phase("ca_rejected").await;
                assert_eq!(sessions(f).await, p.initial_sessions);
                assert_eq!(counter(f).await, initial_counter);
                assert_eq!(immutable, storage::immutable_rows(f).await);
                browser.advance().await;
            }
            browser.phase("ready_login").await;
            p.assert_head_event(&original, 1).await;
            assert_eq!(initial_counter, counter(f).await);
            assert_eq!(sessions(f).await, p.initial_sessions);
            browser
                .send(json!({"otp":super::login_fixture::otp()}))
                .await;
            let logged = browser.phase("authenticated").await;
            let digest = logged["digest"].as_str().unwrap().to_owned();
            exact_session(f, &digest).await;
            let consumed = counter(f).await;
            assert!(consumed > initial_counter);
            let issued_sessions = sessions(f).await;
            assert_ne!(issued_sessions, p.initial_sessions);
            assert_eq!(immutable, storage::immutable_rows(f).await);
            assert_eq!(original_artifacts, storage::artifact_rows(f).await);
            assert_eq!(files, p.files());
            browser.advance().await;

            browser.phase("saved").await;
            let saved = p.current().await;
            storage::assert_saved(&p, &original, &saved, &original_artifacts, &files).await;
            p.assert_head_event(&saved, 2).await;
            assert_eq!(immutable, storage::immutable_rows(f).await);
            assert_eq!(issued_sessions, sessions(f).await);
            let saved_artifacts = storage::artifact_rows(f).await;
            let saved_files = p.files();
            browser.advance().await;

            browser.phase("conflict_ready").await;
            p.assert_head_event(&saved, 2).await;
            let winner = p.admin_advance(&saved).await;
            assert_eq!(u64::from(winner.task.revision), 3);
            assert_eq!(winner.task.title, ADMIN_TITLE);
            p.assert_head_event(&winner, 3).await;
            browser.advance().await;
            browser.phase("conflict").await;
            // Stale full-baseline POST must fail before any new Artifact/file, not merely stop
            // a later PUT. The private unsaved browser draft never becomes server authority.
            p.assert_head_event(&winner, 3).await;
            assert_eq!(saved_artifacts, storage::artifact_rows(f).await);
            assert_eq!(saved_files, p.files());
            assert_eq!(immutable, storage::immutable_rows(f).await);
            assert_eq!(issued_sessions, sessions(f).await);
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
            assert_eq!(saved_artifacts, storage::artifact_rows(f).await);
            assert_eq!(saved_files, p.files());
            assert_eq!(immutable, storage::immutable_rows(f).await);
            assert_eq!(revoked_sessions, sessions(f).await);
            assert_eq!(consumed, counter(f).await);
            storage::exact_profile(f).await;
            browser.advance().await;
            browser.phase("complete").await;
            browser.finish().await;
            p.assert_head_event(&winner, 3).await;
            assert_eq!(saved_artifacts, storage::artifact_rows(f).await);
            assert_eq!(saved_files, p.files());
            assert_eq!(immutable, storage::immutable_rows(f).await);
            assert_eq!(revoked_sessions, sessions(f).await);
            assert!(catalog == f.snapshot().await);
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
