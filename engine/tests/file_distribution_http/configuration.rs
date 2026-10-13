use crate::{fixture::*, *};

#[tokio::test]
async fn distribution_http_configuration_is_inert_canonical_and_has_two_bounded_cached_workspace_budgets(
) {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let before = bytes::snapshot(&f.root);
            assert!(f.pool.is_closed());
            for origin in [
                ORIGIN,
                "https://127.0.0.1",
                "https://platform.example:8443",
                "http://localhost:3000",
                "http://127.0.0.1:3000",
                "http://[::1]:3000",
            ] {
                assert!(FileDistributionHttpState::new(
                    f.source.clone(),
                    f.workspace.clone(),
                    origin
                )
                .is_ok());
            }
            for origin in [
                "",
                "null",
                "*",
                "https://*.example",
                "http://platform.example",
                "http://192.168.1.1",
                "https://PLATFORM.example",
                "https://platform.example/",
                "https://platform.example:443",
                "https://platform.example/path",
                "https://platform.example?q=a",
                "https://platform.example#x",
                "https://user@platform.example",
                " https://platform.example",
                "https://platform.example ",
            ] {
                assert_eq!(
                    FileDistributionHttpState::new(f.source.clone(), f.workspace.clone(), origin)
                        .err(),
                    Some(FileDistributionHttpConfigError::InvalidOrigin)
                );
            }
            let huge_origin = format!("https://{}", "a".repeat(2049));
            assert_eq!(
                FileDistributionHttpState::new(f.source.clone(), f.workspace.clone(), &huge_origin)
                    .err(),
                Some(FileDistributionHttpConfigError::InvalidOrigin)
            );
            for (manifests, chunks) in [(1, 1), (1, 16), (16, 1), (4, 4), (16, 16)] {
                assert!(FileDistributionHttpState::with_capacities(
                    f.source.clone(),
                    f.workspace.clone(),
                    ORIGIN,
                    manifests,
                    chunks
                )
                .is_ok());
            }
            for (manifests, chunks) in [
                (0, 1),
                (1, 0),
                (17, 1),
                (1, 17),
                (usize::MAX, 1),
                (1, usize::MAX),
            ] {
                assert_eq!(
                    FileDistributionHttpState::with_capacities(
                        f.source.clone(),
                        f.workspace.clone(),
                        ORIGIN,
                        manifests,
                        chunks
                    )
                    .err(),
                    Some(FileDistributionHttpConfigError::InvalidCapacity)
                );
            }
            let foreign = DurableAuthSource::new(
                f.pool.clone(),
                "e7b8af81-924e-4f8c-8ce5-f55c7fe337c9".parse().unwrap(),
            );
            assert_eq!(
                FileDistributionHttpState::with_capacities(
                    foreign.clone(),
                    f.workspace.clone(),
                    "null",
                    0,
                    0
                )
                .err(),
                Some(FileDistributionHttpConfigError::InvalidOrigin)
            );
            assert_eq!(
                FileDistributionHttpState::with_capacities(
                    foreign.clone(),
                    f.workspace.clone(),
                    ORIGIN,
                    0,
                    1
                )
                .err(),
                Some(FileDistributionHttpConfigError::InvalidCapacity)
            );
            assert_eq!(
                FileDistributionHttpState::with_capacities(
                    foreign,
                    f.workspace.clone(),
                    ORIGIN,
                    1,
                    1
                )
                .err(),
                Some(FileDistributionHttpConfigError::InvalidWorkspace)
            );
            assert!(f.pool.is_closed());
            assert_eq!(bytes::snapshot(&f.root), before);
            assert!(bytes::tree(&f.root).is_empty());
        })
    })
    .await;
}
