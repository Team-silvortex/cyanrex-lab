use crate::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_manifest_http_uses_final_source_clock_after_real_selected_publication_wait() {
    checked(Fixture::ready().await, |f| Box::pin(async move {
        let route = bounded(f);
        for mode in ["session", "grant-expiry", "grant-arrival"] {
            let mut typed = f.request();
            let boundary = if mode == "session" {
                download_support::expire_soon(f).await;
                None
            } else {
                let boundary = download_support::db_now(f).await + 2;
                let validity = if mode == "grant-arrival" { FileGrantValidity::new(boundary, MAX_FILE_GRANT_UNIX_SECONDS).unwrap() }
                    else { FileGrantValidity::new(0, boundary).unwrap() };
                let seed = if mode == "grant-arrival" { 3 } else { 2 };
                download_support::own_grant(f, seed, validity).await;
                typed = download_support::request(f, seed);
                assert!(download_support::db_now(f).await < boundary);
                Some(boundary)
            };
            let uri = path(&typed);
            let before = f.sharing.snapshot().await;
            let mut held = download_support::pub_holder(f).await;
            let blocker = download_support::backend(&mut held).await;
            let mut pending = Box::pin(route.clone().oneshot(request(&auth(f).target_token, &uri)));
            let mut early = None;
            let waited = tokio::select! {
                result = &mut pending => { early = Some(result.unwrap()); false },
                waited = download_support::blocked(f, blocker) => waited,
            };
            saturated(&route, &uri).await;
            let elapsed = if let Some(boundary) = boundary { waited && download_support::arrived(f, boundary).await }
                else { waited && download_support::expired(f).await };
            held.rollback().await.unwrap();
            let response = match early { Some(response) => response, None => pending.as_mut().await.unwrap() };
            drop(pending);
            assert!(waited && elapsed, "selected Source SQL blocker and database clock, not elapsed sleeps, establish {mode}");
            if mode == "session" {
                error_response(response, StatusCode::UNAUTHORIZED, "invalid_session", "unconfirmed").await;
                assert_eq!(f.sharing.snapshot().await, before);
                download_support::refresh_expiry(f).await;
            } else if mode == "grant-expiry" {
                error_response(response, StatusCode::NOT_FOUND, "not_found", "unconfirmed").await;
                assert_eq!(f.sharing.snapshot().await, before);
            } else {
                assert_eq!(response.status(), StatusCode::OK);
                headers(&response);
                let payload = response.into_body().collect().await.unwrap().to_bytes();
                assert_eq!(FileTransferManifest::parse_json(&payload).unwrap(), f.sharing.base.manifest);
                assert_eq!(f.sharing.snapshot().await, before);
            }
            healthy(f, &route).await;
            download_support::drain(f).await;
        }
    })).await;
}
