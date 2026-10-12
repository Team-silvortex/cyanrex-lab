use super::{fixture::*, pool_gate, support::*, *};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    time::Duration,
};

async fn visible(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let n: i64 = query(&format!(
                "SELECT count(*) AS n FROM {}.{PUBLICATIONS}",
                f.publication_schema
            ))
            .fetch_one(&f.grants.auth.base.admin)
            .await
            .unwrap()
            .get("n");
            if n == 1 {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}
async fn original_source_blocked(f: &Fixture, pid: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2),async {loop{let yes:bool=query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))) AS yes").bind(&f.upload_schema).bind(pid).fetch_one(&f.grants.auth.base.admin).await.unwrap().get("yes");if yes{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_independently_visible_ack_then_original_files_or_records_drift_is_unconfirmed(
) {
    for drift in [
        "inode",
        "root",
        "source-owner",
        "journal-nonce",
        "publication-nonce",
    ] {
        let f = Fixture::ready(17).await;
        f.grants.issue(1, f.actor()).await;
        let before = f.snapshot().await;
        let (publisher, pool, release, started) = pool_gate::publisher(&f).await;
        let workspace = f
            .grants
            .auth
            .source
            .file_publishing_workspace(publisher.clone(), f.grants.workspace.clone())
            .unwrap();
        let mut a = publisher
            .prepare(upload(1), f.actor().principal(), f.actor().account_id())
            .await
            .unwrap();
        release.arm(&pool).await;
        let mut pending = Box::pin(publish_as(
            &f,
            &f.grants.auth.learner_token,
            &workspace,
            1,
            &mut a,
        ));
        tokio::select! {r=&mut pending=>panic!("missing second original source release barrier: {r:?}"),r=tokio::time::timeout(Duration::from_secs(4),started)=>{r.unwrap().unwrap();}}
        let committed = tokio::select! {r=&mut pending=>panic!("post-ACK source connection should remain held: {r:?}"),yes=visible(&f)=>yes};
        // Independent visibility BEFORE any mutation proves a committed row, not a deferred
        // before-COMMIT trigger wait. The Auth source transaction and all grant fences ended.
        let acknowledged = f.snapshot().await;
        let row = f
            .publication
            .get(upload(1), f.actor().principal(), f.actor().account_id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.manifest(), &f.manifest);
        delta(&f, &before, &acknowledged, 1);
        let mut holder = f.grants.auth.base.admin.begin().await.unwrap();
        let pid = backend(&mut holder).await;
        query(&format!(
            "LOCK TABLE {}.collaboration_file_uploads IN ACCESS EXCLUSIVE MODE",
            f.upload_schema
        ))
        .execute(&mut *holder)
        .await
        .unwrap();
        drop(release);
        let waited = tokio::select! {r=&mut pending=>panic!("missing real post-ACK original source SQL wait: {r:?}"),yes=original_source_blocked(&f,pid)=>yes};
        let mut saved = None;
        match drift {
            "inode" => {
                let entry = fs::read_dir(&f.root)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                let retained = f.directory.join("retained-original-chunk");
                fs::rename(&entry, &retained).unwrap();
                let mut replacement = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&entry)
                    .unwrap();
                let data: Vec<_> = (0..17).map(bytes::byte).collect();
                replacement.write_all(&data).unwrap();
                drop(replacement);
                fs::set_permissions(&entry, fs::Permissions::from_mode(0o400)).unwrap();
                assert_ne!(
                    bytes::snapshot(&entry).inode,
                    bytes::snapshot(&retained).inode
                );
                saved = Some((retained, false));
            }
            "root" => {
                let retained = f.directory.join("retained-original-root");
                fs::rename(&f.root, &retained).unwrap();
                fs::DirBuilder::new().mode(0o700).create(&f.root).unwrap();
                for original in fs::read_dir(&retained).unwrap() {
                    let original = original.unwrap().path();
                    let target = f.root.join(original.file_name().unwrap());
                    fs::copy(&original, &target).unwrap();
                    fs::set_permissions(target, fs::Permissions::from_mode(0o400)).unwrap();
                }
                saved = Some((retained, true));
            }
            "source-owner" => {
                query(&format!(
                    "UPDATE {}.collaboration_file_uploads SET owner_id=$1",
                    f.upload_schema
                ))
                .bind(Uuid::new_v4())
                .execute(&mut *holder)
                .await
                .unwrap();
            }
            "journal-nonce" => {
                query(&format!(
                    "UPDATE {}.collaboration_file_upload_chunks SET nonce=$1",
                    f.journal_schema
                ))
                .bind(Uuid::new_v4())
                .execute(&mut *holder)
                .await
                .unwrap();
            }
            _ => {
                query(&format!(
                    "UPDATE {}.{PUBLICATIONS} SET nonce=$1",
                    f.publication_schema
                ))
                .bind(Uuid::new_v4())
                .execute(&mut *holder)
                .await
                .unwrap();
            }
        }
        let retained_snapshot = saved
            .as_ref()
            .map(|(path, directory)| (bytes::snapshot(path), directory.then(|| bytes::tree(path))));
        let injected = snapshot_in_tx(&f, &mut holder).await;
        holder.commit().await.unwrap();
        let result = pending.as_mut().await;
        drop(pending);
        let state = a.state();
        let after = f.snapshot().await;
        let repeated = publish_as(&f, &f.grants.auth.learner_token, &workspace, 1, &mut a).await;
        let repeated_after = f.snapshot().await;
        if let Some((path, directory)) = &saved {
            let (entry, tree) = retained_snapshot.as_ref().unwrap();
            assert_eq!(&bytes::snapshot(path), entry);
            if *directory {
                assert_eq!(&bytes::tree(path), tree.as_ref().unwrap());
            }
        }
        pool.close().await;
        f.cleanup().await;
        assert!(committed && waited);
        assert_eq!(
            result.err(),
            Some(publisher_error(FileUploadPublicationError::Unconfirmed)),
            "{drift}"
        );
        assert_eq!(state, FileUploadPublicationState::Unknown);
        assert_eq!(
            after, injected,
            "the uncertain call must retain the externally injected row/file state"
        );
        assert_eq!(
            repeated.err(),
            Some(publisher_error(FileUploadPublicationError::InvalidInput))
        );
        assert_eq!(repeated_after, after);
    }
}
