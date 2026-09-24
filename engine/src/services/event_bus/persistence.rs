use super::*;
use std::future::Future;
use tokio::sync::oneshot;

impl EventBus {
    pub(crate) fn into_persistence_worker(mut self) -> Self {
        // Otherwise the receiver owns a producer and can never observe the last external drop.
        self.persist_sender = None;
        self
    }

    pub(super) async fn mutation_admission(
        &self,
        username: &str,
    ) -> tokio::sync::OwnedMutexGuard<()> {
        let gate = self
            .mutation_gates
            .lock()
            .expect("event admission registry poisoned")
            .entry(username.to_owned())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone();
        gate.lock_owned().await
    }

    // Publish and mutations share per-owner admission. The writer never takes these locks.
    // This orders one Engine only; cancellation/SQL failure is not rollback or crash recovery.
    pub(super) async fn with_persistence_barrier<T, F, Fut>(&self, username: String, action: F) -> T
    where
        T: Send + 'static,
        F: FnOnce(EventBus) -> Fut + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        let guard = self.mutation_admission(&username).await;
        if self.active_pool().is_none() {
            return action(self.clone()).await;
        }
        let bus = self.clone();
        // Once SQL work is admitted it owns admission through cache publication, even when
        // the HTTP caller leaves. Cancellation while waiting for admission changes nothing.
        tokio::spawn(async move {
            let _guard = guard;
            bus.flush_persistence().await;
            action(bus).await
        })
        .await
        .expect("event mutation worker panicked")
    }

    pub(super) async fn flush_persistence(&self) {
        let (sender, receiver) = oneshot::channel();
        let completed = tokio::time::timeout(StdDuration::from_secs(10), async {
            self.persist_sender
                .as_ref()
                .ok_or(())?
                .send(event_bus_db::PersistMessage::Barrier(sender))
                .await
                .map_err(|_| ())?;
            receiver.await.map_err(|_| ())
        })
        .await;
        if !matches!(completed, Ok(Ok(true))) {
            self.disable_db(
                "persist barrier failed or timed out; using volatile history until restart",
            );
        }
    }

    pub(super) async fn query_with_deadline<T>(
        &self,
        query: impl Future<Output = Result<T, sqlx::Error>>,
    ) -> Result<T, sqlx::Error> {
        match tokio::time::timeout(StdDuration::from_secs(10), query).await {
            Ok(result) => result,
            Err(_) => {
                self.disable_db("event storage operation deadline exceeded; using volatile history until restart");
                Err(sqlx::Error::PoolTimedOut)
            }
        }
    }
}
