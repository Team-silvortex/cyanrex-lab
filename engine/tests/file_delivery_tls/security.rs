use crate::probe::RedirectProbe;
use crate::{fixture::*, support::*, *};
use std::{net::Ipv4Addr, time::Duration};

#[tokio::test]
#[ignore = "requires explicit native OpenSSL loopback TLS fixture"]
async fn openssl_controlled_delivery_rejects_unknown_ca_wrong_ip_san_and_redirect_before_files() {
    for wrong_san in [false, true] {
        let fixture = if wrong_san {
            Fixture::at_ip(17, 5, Ipv4Addr::new(127, 0, 0, 2)).await
        } else {
            Fixture::new(17, 4).await
        };
        let (result, last, closure, requests, tls, source, selected, parents_before, parents_after) =
            checked(fixture, move |f| {
                Box::pin(async move {
                    let target = f.target().unwrap();
                    let client = f.client(wrong_san);
                    let expected = FileVersionRef::from_manifest(&f.manifest);
                    let parents_before = (directory_policy(&f.root), directory_policy(&f.selected));
                    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                    let (observer, mut progress) = FileDeliveryObserver::channel();
                    let result = tokio::time::timeout(
                        Duration::from_secs(30),
                        client.fetch_granted_and_save_controlled(
                            TOKEN,
                            grant(),
                            upload(),
                            &expected,
                            &target,
                            observer,
                            cancellation,
                        ),
                    )
                    .await
                    .map(|result| result.map(|saved| saved.file().clone()));
                    drop(handle);
                    let last = progress.latest();
                    let closure = tokio::time::timeout(Duration::from_secs(10), async {
                        let mut tail = Vec::new();
                        for _ in 0..2 {
                            match progress.changed().await {
                                Some(snapshot) => tail.push(snapshot),
                                None => return (tail, true),
                            }
                        }
                        (tail, false)
                    })
                    .await;
                    drop(progress);
                    drop(target);
                    drop(client);
                    let tls = f.finish_server();
                    (
                        result,
                        last,
                        closure,
                        f.take_requests(),
                        tls,
                        bytes::tree(&f.root),
                        bytes::tree(&f.selected),
                        parents_before,
                        (directory_policy(&f.root), directory_policy(&f.selected)),
                    )
                })
            })
            .await;
        assert_eq!(
            result
                .expect("TLS refusal must finish within the existing bounded request lifetime")
                .err(),
            Some(FileControlledSaveError::Delivery(
                FileGrantedSaveError::Download(FileGrantedDownloadError::Manifest(
                    FileManifestClientError::Unavailable
                ),)
            ))
        );
        assert_eq!(
            tls.received_bytes, 0,
            "TLS rejection before any decrypted HTTP credential or selector"
        );
        assert_eq!(tls.responded, 0);
        assert_eq!(tls.error, None);
        assert!(
            requests.is_empty(),
            "no accepted HTTP request precedes CA/hostname validation"
        );
        assert!(source.is_empty() && selected.is_empty());
        assert_eq!(parents_before, parents_after);
        let last = last.expect("the admitted caller is still only waiting for its manifest");
        assert_eq!(last.phase(), FileDeliveryPhase::FetchingManifest);
        assert_eq!(
            (
                last.staged_chunks(),
                last.staged_bytes(),
                last.total_bytes(),
                last.total_chunks()
            ),
            (0, 0, 17, None)
        );
        let (tail, closed) = closure.expect("TLS refusal closes the one caller observation");
        assert!(closed);
        for snapshot in tail {
            assert_eq!(snapshot, last);
        }
    }

    checked(Fixture::new(17, 6).await, |f| {
        Box::pin(async move {
            let poison = RedirectProbe::start().await;
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.status = axum::http::StatusCode::FOUND;
            reply.headers.insert(
                "location",
                format!("{}/credential-poison", poison.origin())
                    .parse()
                    .unwrap(),
            );
            f.set_reply(Kind::Manifest, reply);
            let outcome = std::panic::AssertUnwindSafe(async {
                let target = f.target().unwrap();
                let client = f.client(true);
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let parents_before = (directory_policy(&f.root), directory_policy(&f.selected));
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let (observer, mut progress) = FileDeliveryObserver::channel();
                let result = within(client.fetch_granted_and_save_controlled(
                    TOKEN,
                    grant(),
                    upload(),
                    &expected,
                    &target,
                    observer,
                    cancellation,
                ))
                .await;
                drop(handle);
                assert_eq!(
                    result.err(),
                    Some(FileControlledSaveError::Delivery(
                        FileGrantedSaveError::Download(FileGrantedDownloadError::Manifest(
                            FileManifestClientError::InvalidResponse
                        ),)
                    ))
                );
                let last = snapshot(
                    f,
                    &progress,
                    FileDeliveryPhase::FetchingManifest,
                    0,
                    0,
                    None,
                );
                closed(&mut progress, Some(last)).await;
                assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&f.selected).is_empty());
                assert_eq!(
                    (directory_policy(&f.root), directory_policy(&f.selected)),
                    parents_before
                );
                let requests = f.take_requests();
                assert_eq!(requests.len(), 1);
                assert_request(&requests[0], &f.manifest, None);
                assert_eq!(
                    poison.hits(),
                    0,
                    "no downgrade/fallback request or credential replay to the second listener"
                );
                let observed = f.finish_server();
                assert!(observed.received_bytes > 0);
                assert_eq!(observed.error, None);
                // A header rejection need not wait for the old peer's complete body write.
                assert!(observed.responded <= 1);
            });
            use futures_util::FutureExt;
            let result = outcome.catch_unwind().await;
            let final_hits = poison.finish().await;
            if let Err(panic) = result {
                std::panic::resume_unwind(panic);
            }
            assert_eq!(
                final_hits, 0,
                "the independent redirect listener drained without any actual HTTP request"
            );
        })
    })
    .await;
}
