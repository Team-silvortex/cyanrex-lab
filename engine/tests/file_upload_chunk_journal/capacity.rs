use super::{bytes::*, control::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_chunk_journal::{FileUploadChunkJournalError, PersistentChunkState},
    file_upload_staging::{
        JournaledFileUploadStaging, JournaledUploadChunkError, UploadChunkState,
    },
};
use sqlx_core::{query::query, row::Row};
use std::sync::atomic::Ordering;

async fn fixture() -> Fixture {
    let fixture = Fixture::new(4, 2, 1).await;
    fixture.journal.install_empty_namespace().await.unwrap();
    for seed in 1..=3 {
        fixture
            .source
            .reserve(upload(seed), owner(1), account(seed), &manifest(17, seed))
            .await
            .unwrap();
    }
    fixture
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_capacity_and_concurrent_coordinate_admission_are_bounded() {
    for confirmed in [false, true] {
        let fixture = fixture().await;
        let source = fixture.rows(false).await;
        let oids = fixture.oids(true).await;
        let mut first = fixture
            .journaled
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        if confirmed {
            fixture
                .journaled
                .stage_chunk(&mut first, Pattern::new(0, 17))
                .await
                .unwrap();
        } else {
            assert!(matches!(
                fixture
                    .journaled
                    .stage_chunk(&mut first, FailedReader)
                    .await,
                Err(JournaledUploadChunkError::Unconfirmed)
            ));
        }
        let before = fixture.rows(true).await;
        let files = tree(&fixture.root);
        for (seed, expected) in [
            (1, FileUploadChunkJournalError::Conflict),
            (2, FileUploadChunkJournalError::CapacityExhausted),
        ] {
            let mut candidate = fixture
                .journaled
                .prepare(upload(seed), owner(1), account(seed), 0)
                .await
                .unwrap();
            let reader = Pattern::new(0, 17);
            let calls = reader.calls.clone();
            assert!(
                matches!(fixture.journaled.stage_chunk(&mut candidate,reader).await,Err(JournaledUploadChunkError::Journal(error)) if error==expected)
            );
            assert_eq!(candidate.state(), UploadChunkState::Unknown);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            assert_eq!(fixture.rows(true).await, before);
            assert_eq!(fixture.rows(false).await, source);
            assert_eq!(tree(&fixture.root), files);
            assert_eq!(fixture.oids(true).await, oids);
        }
        metadata(
            &fixture
                .journal
                .get(upload(1), owner(1), account(1), 0)
                .await
                .unwrap()
                .unwrap(),
            0,
            &fixture.manifest,
            if confirmed {
                PersistentChunkState::Confirmed
            } else {
                PersistentChunkState::Unknown
            },
        );
        assert_eq!(fixture.rows(true).await, before);
        assert_eq!(tree(&fixture.root), files);
        // A real configured-bound-plus-one probe fails closed rather than silently dropping a row.
        query(&format!("INSERT INTO {JOURNAL_ROWS}(upload_id,chunk_index,owner_id,account_id,source_key,root_key,total_length,chunk_length,sha256,nonce,confirmed)
            SELECT $1,chunk_index,owner_id,$2,source_key,root_key,total_length,chunk_length,sha256,$3,FALSE FROM {JOURNAL_ROWS}"))
            .bind(upload(2).as_uuid()).bind(account(2).as_uuid()).bind(uuid::Uuid::new_v4()).execute(&fixture.journal_pool).await.unwrap();
        let injected = fixture.rows(true).await;
        let mut candidate = fixture
            .journaled
            .prepare(upload(3), owner(1), account(3), 0)
            .await
            .unwrap();
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        assert!(matches!(
            fixture.journaled.stage_chunk(&mut candidate, reader).await,
            Err(JournaledUploadChunkError::Journal(
                FileUploadChunkJournalError::InvalidRecord
            ))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.rows(true).await, injected);
        assert_eq!(fixture.rows(false).await, source);
        assert_eq!(tree(&fixture.root), files);
        query(&format!("DELETE FROM {JOURNAL_ROWS} WHERE upload_id=$1"))
            .bind(upload(2).as_uuid())
            .execute(&fixture.journal_pool)
            .await
            .unwrap();
        assert_eq!(fixture.rows(true).await, before);
        fixture.pool_restored(false).await;
        fixture.pool_restored(true).await;
        fixture.cleanup().await;
    }

    let fixture = fixture().await;
    let source = fixture.rows(false).await;
    let journal = fixture.rows(true).await;
    let oids = fixture.oids(true).await;
    // Deliberately poison every application connection's writer defaults, not just the fixture admin.
    let mut connections = Vec::new();
    for _ in 0..4 {
        connections.push(fixture.journal_pool.acquire().await.unwrap());
    }
    for connection in &mut connections {
        query("SET default_transaction_isolation='repeatable read'")
            .execute(&mut **connection)
            .await
            .unwrap();
        query("SET synchronous_commit='off'")
            .execute(&mut **connection)
            .await
            .unwrap();
    }
    drop(connections);
    let body=format!("IF current_setting('transaction_isolation')<>'read committed' OR current_setting('synchronous_commit')<>'on' THEN RAISE EXCEPTION 'writer settings were not frozen before work'; END IF; PERFORM pg_advisory_xact_lock(hashtext('{}'),998);",fixture.journal_schema);
    fault(&fixture, "INSERT", &body, true).await;
    let (holder, pid) = advisory(&fixture).await;
    let independent =
        JournaledFileUploadStaging::new(fixture.local.clone(), fixture.journal.clone()).unwrap();
    let mut first = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let mut second = independent
        .prepare(upload(2), owner(1), account(2), 0)
        .await
        .unwrap();
    let (reader, release, started, finished) = held(0, 17, 0);
    let mut pending = Box::pin(fixture.journaled.stage_chunk(&mut first, reader));
    tokio::select! {result=&mut pending=>panic!("no real first writer barrier: {result:?}"),_=fixture.wait_blocked(true,pid)=>{}}
    let writing_pid:i32=query("SELECT pid FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))")
        .bind(&fixture.journal_schema).bind(pid).fetch_one(&fixture.journal_admin).await.unwrap().get("pid");
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    let mut competing = Box::pin(independent.stage_chunk(&mut second, reader));
    tokio::select! {result=&mut pending=>panic!("first writer escaped barrier: {result:?}"),result=&mut competing=>panic!("second writer did not reach real serialization lock: {result:?}"),_=fixture.wait_blocked(true,writing_pid)=>{}}
    assert_eq!(fixture.rows(true).await, journal);
    assert_eq!(fixture.rows(false).await, source);
    assert!(tree(&fixture.root).is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    holder.rollback().await.unwrap();
    let (_, result) = tokio::join!(
        async {
            tokio::select! {result=&mut pending=>panic!("first stage escaped real reader barrier: {result:?}"),_=signal(started)=>{}}
        },
        competing
    );
    assert!(matches!(
        result,
        Err(JournaledUploadChunkError::Journal(
            FileUploadChunkJournalError::CapacityExhausted
        ))
    ));
    assert_eq!(second.state(), UploadChunkState::Unknown);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(hit(&fixture).await);
    let acknowledged = fixture.rows(true).await;
    assert_eq!(acknowledged[0], journal[0]);
    assert_eq!(acknowledged[1].len(), 1);
    let partial = tree(&fixture.root);
    assert_eq!(partial.len(), 1);
    metadata(
        &fixture
            .journal
            .get(upload(1), owner(1), account(1), 0)
            .await
            .unwrap()
            .unwrap(),
        0,
        &fixture.manifest,
        PersistentChunkState::Unknown,
    );
    assert_eq!(fixture.rows(true).await, acknowledged);
    assert_eq!(tree(&fixture.root), partial);
    drop(release);
    pending.await.unwrap();
    signal(finished).await;
    assert_eq!(first.state(), UploadChunkState::Confirmed);
    assert_eq!(fixture.rows(false).await, source);
    assert_eq!(fixture.oids(true).await, oids);
    assert_eq!(fixture.rows(true).await[1].len(), 1);
    assert_eq!(tree(&fixture.root).len(), 1);
    remove_fault(&fixture).await;
    fixture.pool_restored(false).await;
    fixture.pool_restored(true).await;
    fixture.cleanup().await;
}
