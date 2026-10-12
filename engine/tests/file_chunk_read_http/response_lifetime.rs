use crate::{fixture::*, support::*, *};
use axum::body::HttpBody;
use std::time::Duration;
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_http_binary_response_frame_clones_slices_and_unclaimed_results_retain_both_budgets(
) {
    checked(Fixture::ready(1).await, |f| {
        Box::pin(async move {
            let route = bounded(f);
            let uri = path(&f.request());
            let before = f.sharing.snapshot().await;
            let response = route
                .clone()
                .oneshot(request(&auth(f).target_token, &uri))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            saturated(&route, &uri).await;
            source_full(f).await;
            let mut body = response.into_body();
            let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
            assert_eq!(
                frame.as_ref(),
                &(0..17).map(bytes::byte).collect::<Vec<_>>()
            );
            assert!(body.is_end_stream());
            let clone = frame.clone();
            let slice = clone.slice(1..3);
            drop((body, frame));
            saturated(&route, &uri).await;
            source_full(f).await;
            drop(clone);
            saturated(&route, &uri).await;
            source_full(f).await;
            assert_eq!(slice.as_ref(), &[1, 2]);
            drop(slice);
            healthy(f, &route).await;
            let response = route
                .clone()
                .oneshot(request(&auth(f).target_token, &uri))
                .await
                .unwrap();
            saturated(&route, &uri).await;
            source_full(f).await;
            drop(response);
            healthy(f, &route).await;
            // Keep a completed task's response UNCLAIMED. Its stored Body still owns both leases.
            let input = request(&auth(f).target_token, &uri);
            let unclaimed = tokio::spawn(route.clone().oneshot(input));
            tokio::time::timeout(Duration::from_secs(3), async {
                while !unclaimed.is_finished() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            saturated(&route, &uri).await;
            source_full(f).await;
            drop(unclaimed);
            healthy(f, &route).await;
            let response = route
                .clone()
                .oneshot(request(&auth(f).target_token, &uri))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let mut late = response.into_body();
            tokio::time::sleep(Duration::from_millis(10_100)).await;
            // There is deliberately no autonomous timer destroying an unpolled verified owner.
            saturated(&route, &uri).await;
            source_full(f).await;
            let frame = late
                .frame()
                .await
                .expect("expired first poll must yield a body error");
            assert!(
                frame.is_err(),
                "late first poll must not disclose any Data frame"
            );
            healthy(f, &route).await;
            drop(late);
            assert_eq!(f.sharing.snapshot().await, before);
            drain(f).await;
        })
    })
    .await;
}
