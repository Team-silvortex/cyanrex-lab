use super::{
    child::{safe_threads, stopped},
    fixture::*,
    partial_storage as state, partial_waits as waits,
    process::Browser,
    storage,
};
use serde_json::json;

#[tokio::test]
#[ignore = "requires explicit disposable SQL, browser and non-root capless children"]
async fn postgres_platform_browser_task_body_partial_prefix_second_lock_timeout_retains_confirmed_artifact(
) {
    exercise(false).await;
}

#[tokio::test]
#[ignore = "requires explicit disposable SQL, browser and non-root capless children"]
async fn postgres_platform_browser_task_body_partial_prefix_second_session_revoked_retains_confirmed_artifact(
) {
    exercise(true).await;
}

async fn exercise(revoked: bool) {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f, false).await;
            storage::exact_profile(f).await;
            let catalog = f.snapshot().await;
            let original = p.current().await;
            p.assert_head_event(&original, 1).await;
            let tasks = state::task_rows(f).await;
            let artifacts = storage::artifact_rows(f).await;
            let files = p.files();
            let initial_counter = counter(f).await;
            let mut host = p.spawn();
            assert!(host.ready().await);
            assert!(safe_threads(host.pid(), &p.identity));
            let mut input = p.input();
            input["revoked"] = json!(revoked);
            let mut browser = Browser::start_partial(input).await;
            browser.phase("ready_login").await;
            assert_eq!(sessions(f).await, p.initial_sessions);
            assert_eq!(counter(f).await, initial_counter);
            browser
                .send(json!({"otp":super::login_fixture::otp()}))
                .await;
            let login = browser.phase("authenticated").await;
            let digest = login["digest"].as_str().unwrap().to_owned();
            exact_session(f, &digest).await;
            let consumed = counter(f).await;
            assert!(consumed > initial_counter);
            let issued_sessions = sessions(f).await;
            assert_ne!(issued_sessions, p.initial_sessions);
            let immutable = storage::immutable_rows(f).await;
            assert_eq!(tasks, state::task_rows(f).await);
            assert_eq!(artifacts, storage::artifact_rows(f).await);
            assert_eq!(files, p.files());
            browser.advance().await;
            browser.phase("prepared").await;

            // Hold first publication before bytes. The next dedicated row waiter is proven
            // queued behind that exact platform transaction, not raced against an HTTP callback.
            let gate = waits::task_gate(f).await;
            browser.advance().await;
            let first = waits::platform_wait(f, gate.pid, waits::TASK_STATEMENT).await;
            let queued = waits::queue_after_first(f, first, &digest, revoked).await;
            assert_ne!(queued.pid, first);
            assert_eq!(artifacts, storage::artifact_rows(f).await);
            assert_eq!(files, p.files());
            gate.release().await;
            let next = queued.acquire().await;
            let second = waits::platform_wait(
                f,
                next.pid,
                if revoked {
                    waits::SESSION_STATEMENT
                } else {
                    waits::TASK_STATEMENT
                },
            )
            .await;
            assert_ne!(first, second);
            let prefix = browser.phase("first_confirmed").await;
            // Ordinary owner MVCC queries cannot self-block on the selected Session/Task fence.
            assert_eq!(tasks, state::task_rows(f).await);
            state::assert_prefix(
                &p,
                &original,
                &artifacts,
                &files,
                &prefix["reference"],
                &prefix["pending"],
            )
            .await;
            if revoked {
                // The actual naturally issued Cookie's exact Session is legally removed while
                // its second request is already waiting. No permission/schema mutation or kill.
                next.revoke(f, &digest).await;
            } else {
                // Keep this owner Task row lock until the unchanged product lock_timeout
                // produces a real 503. No browser deadline extension or manufactured response.
                browser.advance().await;
                browser.phase("terminal").await;
                next.release().await;
            }
            if revoked {
                browser.advance().await;
                browser.phase("terminal").await;
            }
            let final_sessions = sessions(f).await;
            if revoked {
                assert_ne!(final_sessions, issued_sessions);
                assert_eq!(
                    final_sessions, p.initial_sessions,
                    "only the naturally issued Session may be removed"
                );
            } else {
                assert_eq!(final_sessions, issued_sessions);
                exact_session(f, &digest).await;
            }
            p.assert_head_event(&original, 1).await;
            assert_eq!(tasks, state::task_rows(f).await);
            state::assert_prefix(
                &p,
                &original,
                &artifacts,
                &files,
                &prefix["reference"],
                &prefix["pending"],
            )
            .await;
            assert_eq!(immutable, storage::immutable_rows(f).await);
            assert_eq!(counter(f).await, consumed);
            storage::exact_profile(f).await;
            let retained_artifacts = storage::artifact_rows(f).await;
            let retained_files = p.files();
            browser.advance().await;
            browser.phase("complete").await;
            browser.finish().await;
            assert_eq!(retained_artifacts, storage::artifact_rows(f).await);
            assert_eq!(retained_files, p.files());
            assert_eq!(tasks, state::task_rows(f).await);
            assert_eq!(immutable, storage::immutable_rows(f).await);
            assert_eq!(final_sessions, sessions(f).await);
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
