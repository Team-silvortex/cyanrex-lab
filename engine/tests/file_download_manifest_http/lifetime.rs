use crate::{fixture::*, support::*, *};
use axum::body::HttpBody;
use std::time::Duration;
struct Unclaimed(Option<tokio::task::JoinHandle<axum::response::Response>>);
impl Drop for Unclaimed {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}
async fn restored(f: &Fixture, route: &Router, uri: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let response = route
                .clone()
                .oneshot(request(&auth(f).target_token, uri))
                .await
                .unwrap();
            if response.status() == StatusCode::OK {
                headers(&response);
                let payload = response.into_body().collect().await.unwrap().to_bytes();
                assert_eq!(
                    FileTransferManifest::parse_json(&payload).unwrap(),
                    f.sharing.base.manifest
                );
                break;
            }
            error_response(
                response,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("dropping an unclaimed completed response must restore HTTP admission");
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_manifest_http_sql_wait_cancel_and_json_response_lifetime_do_not_create_byte_leases_or_fresh_authority(
) {
    checked(Fixture::ready().await, |f| Box::pin(async move {
        let route = bounded(f);
        let uri = path(&f.request());
        let before = f.sharing.snapshot().await;
        let mut held = download_support::pub_holder(f).await;
        let blocker = download_support::backend(&mut held).await;
        let mut pending = Box::pin(route.clone().oneshot(request(&auth(f).target_token, &uri)));
        let mut early = None;
        let waited = tokio::select! {
            response = &mut pending => { early = Some(response.unwrap()); false },
            waited = download_support::blocked(f, blocker) => waited,
        };
        saturated(&route, &uri).await;
        drop(pending);
        held.rollback().await.unwrap();
        drop(early);
        download_support::drain(f).await;
        assert!(waited, "cancel only while a real selected publication lock wait is observed");
        assert_eq!(f.sharing.snapshot().await, before);
        healthy(f, &route).await;
        let response = route.clone().oneshot(request(&auth(f).target_token, &uri)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        saturated(&route, &uri).await;
        assert_eq!(auth(f).source.get_session_file_download_manifest(&auth(f).target_token, &f.workspace, &f.request()).await.unwrap(), f.sharing.base.manifest,
            "an unsent HTTP response does not occupy a Source byte/work budget");
        let mut body = response.into_body();
        let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
        let retained = frame.clone();
        assert_eq!(FileTransferManifest::parse_json(&retained).unwrap(), f.sharing.base.manifest);
        assert!(body.is_end_stream());
        healthy(f, &route).await;
        let response = route.clone().oneshot(request(&auth(f).target_token, &uri)).await.unwrap();
        saturated(&route, &uri).await;
        drop(response);
        healthy(f, &route).await;
        let selected = request(&auth(f).target_token, &uri);
        let unclaimed_route = route.clone();
        let unclaimed = Unclaimed(Some(tokio::spawn(async move { unclaimed_route.oneshot(selected).await.unwrap() })));
        tokio::time::timeout(Duration::from_secs(3), async {
            while !unclaimed.0.as_ref().unwrap().is_finished() { tokio::task::yield_now().await; }
        }).await.unwrap();
        saturated(&route, &uri).await;
        drop(unclaimed);
        restored(f, &route, &uri).await;
        assert_eq!(f.sharing.snapshot().await, before);
        drop((body, frame, retained));
        let response = route.clone().oneshot(request(&auth(f).target_token, &uri)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        f.sharing.store.revoke(f.request().grant_id, f.sharing.actor(), sharing_fixture::revision(1)).await.unwrap();
        let revoked = f.sharing.snapshot().await;
        let independent = app(f);
        denied(f, &independent, request(&auth(f).target_token, &uri), StatusCode::NOT_FOUND, "not_found").await;
        let payload = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(FileTransferManifest::parse_json(&payload).unwrap(), f.sharing.base.manifest,
            "body polling discloses the prior committed metadata snapshot without reauthorizing it");
        denied(f, &route, request(&auth(f).target_token, &uri), StatusCode::NOT_FOUND, "not_found").await;
        assert_eq!(f.sharing.snapshot().await, revoked);
        download_support::drain(f).await;
    })).await;
}
