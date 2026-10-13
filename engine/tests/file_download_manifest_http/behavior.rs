use crate::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_manifest_http_preserves_empty_and_cross_chunk_metadata_without_publisher_permission_dependency(
) {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let route = app(f);
            for (seed, length) in [(2, 0), (3, FILE_TRANSFER_CHUNK_BYTES + 17)] {
                let (manifest, request) = additional(f, seed, length).await;
                assert_eq!(
                    manifest.chunk_sha256().len(),
                    if length == 0 { 0 } else { 2 }
                );
                if length != 0 {
                    assert_ne!(
                        manifest.chunk(0).unwrap().sha256,
                        manifest.chunk(1).unwrap().sha256
                    );
                    assert_eq!(manifest.chunk(1).unwrap().offset, FILE_TRANSFER_CHUNK_BYTES);
                    assert_eq!(manifest.chunk(1).unwrap().byte_length, 17);
                }
                success(f, &route, &request, &manifest).await;
            }
            f.sharing
                .base
                .grants
                .store
                .revoke(
                    grant_fixture::grant(1),
                    f.sharing.base.grants.issuer(),
                    sharing_fixture::revision(1),
                )
                .await
                .unwrap();
            healthy(f, &route).await;
            download_support::drain(f).await;
        })
    })
    .await;
}
