use super::{fixture::*, *};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStaging,
    file_upload_staging::FileUploadStaging,
    file_upload_store::{FileUploadLimits, FileUploadStore},
};
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
        self.arm_count(pool, 2).await;
    }
    pub async fn arm_count(&self, pool: &PgPool, n: usize) {
        assert!(matches!(n, 1 | 2));
        tokio::time::timeout(Duration::from_secs(3), async {
            while pool.num_idle() != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("trusted prepare returned its sole source connection before arming");
        self.0.remaining.store(n, Ordering::SeqCst);
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        self.0.released.notify_one();
    }
}

/// Hold return of the second original-source short COMMIT after prepare (fresh bytes completed).
/// Final authority INSERT uses the independent Auth source transaction, so its committed Published
/// row can be observed while the next original-source post-ACK check waits for this connection.
pub async fn publisher(
    f: &Fixture,
) -> (FileUploadPublisher, PgPool, Release, oneshot::Receiver<()>) {
    let (entered, started) = oneshot::channel();
    let gate = Arc::new(Gate {
        remaining: AtomicUsize::new(0),
        entered: Mutex::new(Some(entered)),
        released: Notify::new(),
    });
    let callback = gate.clone();
    let schema = f.upload_schema.clone();
    let pool=PgPoolOptions::new().max_connections(1).after_connect(move|c,_|{let schema=schema.clone();Box::pin(async move {query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)").bind(schema).execute(c).await?;Ok(())})})
        .after_release(move|_,_|{let held=callback.remaining.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|(n>0).then(||n-1)).ok()==Some(1);let gate=held.then(||callback.clone());Box::pin(async move {if let Some(g)=gate {if let Some(tx)=g.entered.lock().unwrap().take(){let _=tx.send(());}g.released.notified().await;}Ok(true)})})
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database")).await.unwrap();
    let installation_id: Uuid = query(&format!(
        "SELECT installation_id FROM {}.collaboration_file_upload_schema",
        f.upload_schema
    ))
    .fetch_one(&f.grants.auth.base.admin)
    .await
    .unwrap()
    .get("installation_id");
    let uploads = FileUploadStore::new(
        pool.clone(),
        scope(),
        installation_id,
        &f.upload_schema,
        FileUploadLimits::new(MAX_FILE_TRANSFER_BYTES, MAX_FILE_TRANSFER_BYTES, 16, 8).unwrap(),
    )
    .unwrap();
    let staging = ChunkStaging::open(scope(), &f.root, 2).unwrap();
    let local = FileUploadStaging::new(uploads, staging, 4).unwrap();
    let publisher =
        FileUploadPublisher::new(local, f.journal.clone(), f.publication.clone()).unwrap();
    (publisher, pool, Release(gate), started)
}
