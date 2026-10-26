#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL, explicit Playwright/Chromium paths and non-root capless child support"]
async fn postgres_platform_browser_plain_real_session_cookie_otp_confirmation_and_storage() {
    super::process::exercise(false).await;
}
