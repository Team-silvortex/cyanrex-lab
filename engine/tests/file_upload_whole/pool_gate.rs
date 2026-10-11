use super::fixture::*;
use cyanrex_engine::services::{
    file_upload_staging::FileUploadStaging, file_upload_store::FileUploadStore,
};
use sqlx_core::query::query;
use sqlx_postgres::{PgPool, PgPoolOptions};
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

/// Only the test pool's first COMMIT/connection-return is intercepted. No production hook or timer.
pub async fn captured_pool(
    fixture: &Fixture,
    controls: usize,
) -> (FileUploadStaging, PgPool, Release, oneshot::Receiver<()>) {
    let (entered, started) = oneshot::channel();
    let gate = Arc::new(Gate {
        armed: AtomicBool::new(false),
        entered: Mutex::new(Some(entered)),
        released: Notify::new(),
    });
    let callback = gate.clone();
    let selected = fixture.schema.clone();
    let pool=PgPoolOptions::new().max_connections(1)
        .after_connect(move |connection,_| {let selected=selected.clone();Box::pin(async move {
            query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)").bind(selected).execute(connection).await?;Ok(())
        })})
        .after_release(move |_,_| {let held=callback.armed.swap(false,Ordering::SeqCst).then(||callback.clone());Box::pin(async move {
            if let Some(held)=held {
                if let Some(entered)=held.entered.lock().unwrap().take() {let _=entered.send(());}
                held.released.notified().await;
            }Ok(true)
        })})
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database required")).await.unwrap();
    // The connection is observed idle before arming, so initialization cannot consume the gate.
    tokio::time::timeout(Duration::from_secs(2), async {
        while pool.num_idle() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("test pool did not finish its initialization release");
    let source = FileUploadStore::new(
        pool.clone(),
        scope(),
        fixture.installation_id,
        &fixture.schema,
        limits(),
    )
    .unwrap();
    let adapter = FileUploadStaging::new(source, fixture.staging.clone(), controls).unwrap();
    gate.armed.store(true, Ordering::SeqCst);
    (adapter, pool, Release(gate), started)
}
