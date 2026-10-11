use super::fixture::*;
use cyanrex_engine::services::{
    file_upload_chunk_journal::FileUploadChunkJournal,
    file_upload_staging::{FileUploadPublisher, FileUploadStaging},
    file_upload_store::FileUploadStore,
};
use sqlx_core::query::query;
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{oneshot, Notify};

struct Gate {
    remaining: AtomicUsize,
    entered: Mutex<Option<oneshot::Sender<()>>>,
    released: Notify,
}
pub struct Release(Arc<Gate>);
impl Release {
    pub async fn arm(&self, pool: &PgPool) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while pool.num_idle() != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("prepare capture returned its sole connection before arming");
        self.0.remaining.store(2, Ordering::SeqCst);
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        self.0.released.notify_one();
    }
}

/// Intercepts the second source/journal COMMIT return after prepare (fresh whole-byte pre-INSERT
/// verification). The next use of this one-connection pool is the post-publication-ACK recheck.
pub async fn publisher(
    f: &Fixture,
    store: Store,
) -> (FileUploadPublisher, PgPool, Release, oneshot::Receiver<()>) {
    assert!(matches!(store, Store::Source | Store::Journal));
    let (entered, started) = oneshot::channel();
    let gate = Arc::new(Gate {
        remaining: AtomicUsize::new(0),
        entered: Mutex::new(Some(entered)),
        released: Notify::new(),
    });
    let callback = gate.clone();
    let selected = f.schema(store).to_owned();
    let pool=PgPoolOptions::new().max_connections(1)
        .after_connect(move|connection,_|{let selected=selected.clone();Box::pin(async move{
            query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)").bind(selected).execute(connection).await?;Ok(())
        })})
        .after_release(move|_,_|{
            let held=callback.remaining.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|(n>0).then(||n-1)).ok()==Some(1);
            let gate=held.then(||callback.clone());Box::pin(async move{if let Some(gate)=gate{
                if let Some(entered)=gate.entered.lock().unwrap().take(){let _=entered.send(());}
                gate.released.notified().await;
            }Ok(true)})
        }).connect(&std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database")).await.unwrap();
    let (source, journal) = match store {
        Store::Source => (
            FileUploadStore::new(
                pool.clone(),
                scope(),
                f.source_installation_id,
                &f.source_schema,
                limits(),
            )
            .unwrap(),
            f.journal.clone(),
        ),
        Store::Journal => (
            f.source.clone(),
            FileUploadChunkJournal::new(
                pool.clone(),
                scope(),
                f.journal_installation_id,
                &f.journal_schema,
                f.entries,
            )
            .unwrap(),
        ),
        Store::Publication => unreachable!(),
    };
    let local = FileUploadStaging::new(source, f.staging.clone(), 4).unwrap();
    let publisher = FileUploadPublisher::new(local, journal, f.publication.clone()).unwrap();
    (publisher, pool, Release(gate), started)
}
