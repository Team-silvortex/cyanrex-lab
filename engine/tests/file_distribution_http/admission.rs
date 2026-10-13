use crate::{fixture::*, support::*, *};

#[tokio::test]
async fn distribution_http_default_four_and_explicit_asymmetric_budgets_are_independent_shared_by_clones_and_additive_between_states(
) {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let clone = build_file_distribution_router(f.state.clone());
            let mut held = Vec::new();
            for index in [None, Some(0)] {
                for _ in 0..4 {
                    held.push(Held::start(f, f.app.clone(), index).await);
                }
                saturated(f, &clone, index).await;
            }
            let independent = build_file_distribution_router(
                FileDistributionHttpState::new(f.source.clone(), f.workspace.clone(), ORIGIN)
                    .unwrap(),
            );
            for index in [None, Some(0)] {
                available(f, &independent, index).await;
                saturated(f, &clone, index).await;
            }
            for request in held {
                request.cancel().await;
            }
            for index in [None, Some(0)] {
                available(f, &clone, index).await;
            }

            let state = f.bounded(1, 2);
            let first = build_file_distribution_router(state.clone());
            let second = build_file_distribution_router(state);
            let manifest = Held::start(f, first.clone(), None).await;
            saturated(f, &second, None).await;
            let chunk0 = Held::start(f, first.clone(), Some(0)).await;
            let chunk1 = Held::start(f, second.clone(), Some(0)).await;
            saturated(f, &first, Some(0)).await;
            manifest.cancel().await;
            available(f, &second, None).await;
            saturated(f, &second, Some(0)).await;
            chunk0.cancel().await;
            chunk1.cancel().await;
            available(f, &first, Some(0)).await;
            assert!(bytes::tree(&f.root).is_empty());
        })
    })
    .await;
}
