use super::{fixture::*, publication::Published, support::*, wait_support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_metadata_row_and_marker_waits_recheck_freshness_expiry_and_cancel() {
    for publication in [false, true] {
        for marker in [false, true] {
            let f = Fixture::ready().await;
            let published = if publication {
                Some(Published::empty(&f, &f.auth.learner_token, 1).await)
            } else {
                reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1))
                    .await
                    .unwrap();
                None
            };
            let original = read(&f, publication).await.unwrap();
            let mut held = holder(&f, publication, marker).await;
            let pid = backend(&mut held).await;
            let mut pending = Box::pin(read(&f, publication));
            let mut early = None;
            let waited =
                tokio::select! {r=&mut pending=>{early=Some(r);false},_=f.wait_blocked(pid)=>true};
            let schema = if publication {
                &f.publication_schema
            } else {
                &f.upload_schema
            };
            let table = if publication {
                PUBLICATION_ROWS
            } else {
                UPLOAD_ROWS
            };
            let changed = FileTransferManifest::new(
                scope(),
                original.manifest().file_id(),
                original.manifest().version_id(),
                "fresh-after-row-wait.bin",
                original.manifest().byte_length(),
                original.manifest().chunk_sha256().to_vec(),
                original.manifest().sha256().clone(),
            )
            .unwrap();
            if marker {
                let table = if publication {
                    PUBLICATION_MARKER
                } else {
                    UPLOAD_MARKER
                };
                query(&format!("UPDATE {schema}.{table} SET installation_id=$1"))
                    .bind(Uuid::new_v4())
                    .execute(&mut *held)
                    .await
                    .unwrap();
            } else {
                query(&format!(
                    "UPDATE {schema}.{table} SET manifest=$1 WHERE upload_id=$2"
                ))
                .bind(serde_json::to_vec(&changed).unwrap())
                .bind(upload(1).as_uuid())
                .execute(&mut *held)
                .await
                .unwrap();
            }
            let expected = snapshot_in_tx(&f, &mut held).await;
            held.commit().await.unwrap();
            let result = match early {
                Some(r) => r,
                None => pending.as_mut().await,
            };
            drop(pending);
            let after = snapshot(&f).await;
            // Restore the explicitly injected value and prove healthy selected metadata remains usable.
            if marker {
                let table = if publication {
                    PUBLICATION_MARKER
                } else {
                    UPLOAD_MARKER
                };
                let identity = if publication {
                    f.publication_installation_id
                } else {
                    f.upload_installation_id
                };
                query(&format!("UPDATE {schema}.{table} SET installation_id=$1"))
                    .bind(identity)
                    .execute(&f.auth.base.admin)
                    .await
                    .unwrap();
            } else {
                query(&format!(
                    "UPDATE {schema}.{table} SET manifest=$1 WHERE upload_id=$2"
                ))
                .bind(serde_json::to_vec(original.manifest()).unwrap())
                .bind(upload(1).as_uuid())
                .execute(&f.auth.base.admin)
                .await
                .unwrap();
            }
            let healthy = read(&f, publication).await;
            f.pool_restored().await;
            if let Some(p) = published {
                p.cleanup(&f).await;
            }
            f.cleanup().await;
            assert!(waited,"GET must wait on its exact selected row/marker FOR SHARE; an earlier plain schema SELECT is insufficient");
            assert_eq!(
                after, expected,
                "GET cannot change an injection observed before its writer committed"
            );
            if marker {
                let error = if publication {
                    SessionFileMetadataError::Publication(
                        FileUploadPublicationStoreError::IncarnationMismatch,
                    )
                } else {
                    SessionFileMetadataError::Upload(FileUploadStoreError::IncarnationMismatch)
                };
                assert_eq!(result, Err(error));
            } else {
                assert_eq!(
                    result.unwrap().manifest(),
                    &changed,
                    "selected row is read fresh after a real row lock wait"
                );
            }
            assert_eq!(healthy.unwrap(), original);
        }
        // Expiry is set before entering the real row wait (the reader already locks its Session).
        let f = Fixture::ready().await;
        let published = if publication {
            Some(Published::empty(&f, &f.auth.learner_token, 1).await)
        } else {
            reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1))
                .await
                .unwrap();
            None
        };
        let mut held = holder(&f, publication, false).await;
        let pid = backend(&mut held).await;
        expire_soon(&f).await;
        let before = snapshot(&f).await;
        let mut pending = Box::pin(read(&f, publication));
        let mut early = None;
        let waited =
            tokio::select! {r=&mut pending=>{early=Some(r);false},_=f.wait_blocked(pid)=>true};
        let elapsed = waited && expired(&f).await;
        held.rollback().await.unwrap();
        let result = match early {
            Some(r) => r,
            None => pending.as_mut().await,
        };
        drop(pending);
        let after = snapshot(&f).await;
        refresh_expiry(&f).await;
        let healthy = read(&f, publication).await;
        f.pool_restored().await;
        if let Some(p) = published {
            p.cleanup(&f).await;
        }
        f.cleanup().await;
        assert!(waited && elapsed);
        assert_eq!(
            result,
            Err(SessionFileMetadataError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        assert_eq!(after, before);
        assert!(healthy.is_ok());
        // Cancellation occurs only after the independent backend actually observes the blocked read.
        let f = Fixture::ready().await;
        let published = if publication {
            Some(Published::empty(&f, &f.auth.learner_token, 1).await)
        } else {
            reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1))
                .await
                .unwrap();
            None
        };
        let before = snapshot(&f).await;
        let original = read(&f, publication).await.unwrap();
        let mut held = holder(&f, publication, true).await;
        let pid = backend(&mut held).await;
        let mut pending = Box::pin(read(&f, publication));
        let waited = tokio::select! {_=&mut pending=>false,_=f.wait_blocked(pid)=>true};
        drop(pending);
        held.rollback().await.unwrap();
        if publication {
            drain_publication(&f).await;
        } else {
            drain(&f).await;
        }
        let after = snapshot(&f).await;
        let healthy = read(&f, publication).await;
        if let Some(p) = published {
            p.cleanup(&f).await;
        }
        f.cleanup().await;
        assert!(waited);
        assert_eq!(after, before);
        assert_eq!(healthy.unwrap(), original);
    }
    // Cancel a real deferred INSERT wait: provisional row/source locks roll back, not merely GET.
    let f = Fixture::ready().await;
    fault(
        &f,
        &format!(
            "PERFORM pg_advisory_xact_lock(hashtext('{}'),778);",
            f.upload_schema
        ),
        true,
    )
    .await;
    let (holder, pid) = advisory(&f).await;
    let before = snapshot(&f).await;
    let m = manifest(17, 1);
    let mut pending = Box::pin(f.auth.source.reserve_session_file_upload(
        &f.auth.learner_token,
        &f.upload_workspace,
        upload(1),
        &m,
    ));
    let waited = tokio::select! {_=&mut pending=>false,_=f.wait_blocked(pid)=>true};
    drop(pending);
    holder.rollback().await.unwrap();
    drain(&f).await;
    let after = snapshot(&f).await;
    let reached = hit(&f).await;
    remove_fault(&f).await;
    let healthy = reserve(&f, &f.auth.learner_token, 1, &m).await;
    f.cleanup().await;
    assert!(waited && reached);
    assert_eq!(after, before);
    assert!(healthy.is_ok());
}
