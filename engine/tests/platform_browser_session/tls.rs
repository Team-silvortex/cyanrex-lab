#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL, explicit Playwright/Chromium paths, OpenSSL/certutil and isolated XDG NSS trust without legacy user NSS"]
async fn postgres_platform_browser_tls_private_ca_real_cookie_session_and_storage() {
    super::process::exercise(true).await;
}
