use super::{
    child::{safe_threads, stopped},
    fixture::*,
    process::Browser,
};
use serde_json::json;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL, explicit Playwright/Chromium paths and non-root capless child support"]
async fn postgres_platform_browser_tasks_real_private_pages_reopen_and_revocation_never_write() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            no_resource_writes(f).await;
            let original = resources(f).await;
            let files = p.files();
            let original_counter = counter(f).await;
            let mut host = p.spawn();
            assert!(host.ready().await);
            assert!(safe_threads(host.pid(), &p.identity));
            let mut browser = Browser::start(p.browser_input()).await;
            browser.phase("ready_login").await;
            assert_eq!(original, resources(f).await);
            assert_eq!(files, p.files());
            assert_eq!(original_counter, counter(f).await);
            browser
                .send(json!({"otp":super::login_fixture::otp()}))
                .await;
            let login = browser.phase("authenticated").await;
            let digest = login["digest"].as_str().unwrap().to_owned();
            exact_session(f, &digest).await;
            let consumed = counter(f).await;
            assert!(consumed > original_counter);
            assert_eq!(original, resources(f).await);
            assert_eq!(files, p.files());
            browser.advance().await;
            browser.phase("read_complete").await;
            exact_session(f, &digest).await;
            assert_eq!(consumed, counter(f).await);
            assert_eq!(original, resources(f).await);
            assert_eq!(files, p.files());
            no_resource_writes(f).await;
            // Only trusted fixture administration changes policy. Observe a new baseline after
            // that explicit mutation; the subsequent denied UI reads must change nothing else.
            p.base.suspend(0).await;
            let denied = resources(f).await;
            browser.advance().await;
            browser.phase("denied").await;
            assert_eq!(denied, resources(f).await);
            assert_eq!(files, p.files());
            assert_eq!(consumed, counter(f).await);
            exact_session(f, &digest).await;
            no_resource_writes(f).await;
            browser.advance().await;
            browser.phase("complete").await;
            browser.finish().await;
            // Closing a browser is not logout: the naturally issued Session remains committed.
            exact_session(f, &digest).await;
            assert_eq!(denied, resources(f).await);
            assert_eq!(files, p.files());
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
