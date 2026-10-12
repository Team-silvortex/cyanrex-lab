use crate::{fixture::Fixture, support::auth, *};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{oneshot, Notify};
struct Gate {
    armed: AtomicBool,
    entered: Mutex<Option<oneshot::Sender<()>>>,
    released: Notify,
}
pub struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.released.notify_one();
    }
}
pub async fn captured(
    f: &Fixture,
) -> (
    DurableAuthSource,
    SessionFileChunkReadWorkspace,
    Release,
    oneshot::Receiver<()>,
    PgPool,
) {
    let (entered, started) = oneshot::channel();
    let gate = Arc::new(Gate {
        armed: AtomicBool::new(false),
        entered: Mutex::new(Some(entered)),
        released: Notify::new(),
    });
    let callback = gate.clone();
    let schema = auth(f).base.schema.clone();
    let pool=PgPoolOptions::new().max_connections(1).after_connect(move |c,_|{let schema=schema.clone();Box::pin(async move {
        query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)").bind(schema).execute(c).await?;Ok(())})})
        .after_release(move |_,_|{let held=callback.armed.swap(false,Ordering::SeqCst).then(||callback.clone());Box::pin(async move {
            if let Some(g)=held {if let Some(tx)=g.entered.lock().unwrap().take(){let _=tx.send(());}g.released.notified().await;}Ok(true)})})
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database required")).await.unwrap();
    f.track(pool.clone());
    tokio::time::timeout(Duration::from_secs(3), async {
        while pool.num_idle() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("initial pool release must settle before arming");
    let source = DurableAuthSource::new(pool.clone(), authority());
    let downloads = source
        .file_download_workspace(
            f.sharing.base.publication.clone(),
            f.sharing.recipient_workspace.clone(),
        )
        .unwrap();
    let workspace = source
        .file_chunk_read_workspace(downloads, f.staging.clone(), f.capacity)
        .unwrap();
    gate.armed.store(true, Ordering::SeqCst);
    (source, workspace, Release(gate), started, pool)
}
// after_release proves the ORIGINAL metadata transaction committed and released locks. It does
// not expose a file-job callback or itself establish a mid-read filesystem mutation boundary.
