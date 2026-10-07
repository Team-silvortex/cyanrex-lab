use super::super::DurableAuthSource;
use super::*;
use crate::sqlx_compat::{query, Row};
use std::time::Duration;

#[allow(dead_code)]
#[path = "otp_sql_support.rs"]
mod support;
use support::*;

#[path = "login_password_sql_behavior.rs"]
mod behavior;

struct BlockingRelease(Option<std::sync::mpsc::Sender<()>>);
impl Drop for BlockingRelease {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

fn isolated_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap()
}

#[test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
fn postgres_login_password_missing_account_queues_work_without_holding_source_resources() {
    isolated_runtime().block_on(async {
        let f = Fixture::ready().await;
        let before = f.snapshot().await;
        let pool = f.separate_pool("missing-login-password-boundary").await;
        let source = f.independent_source(pool.clone());
        let gate = shared_gate();
        let baseline = (
            gate.workers.available_permits(),
            gate.total.available_permits(),
        );
        let (release, wait) = std::sync::mpsc::channel();
        let release = BlockingRelease(Some(release));
        let (entered, started) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = entered.send(());
            wait.recv_timeout(Duration::from_secs(10)).is_ok()
        });
        let executor_held = tokio::time::timeout(Duration::from_secs(2), started)
            .await
            .is_ok();
        let pending = tokio::spawn(async move {
            source
                .login(
                    &"missing-password-learner".parse().unwrap(),
                    PASSWORD,
                    "000000",
                )
                .await
                .map(|_| ())
        });
        // A semaphore observation establishes real admission; idle COMMIT alone would only
        // establish snapshot completion, not dispatch into the occupied blocking executor.
        let dispatched = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if pending.is_finished() {
                    return false;
                }
                if gate.workers.available_permits() + 1 == baseline.0
                    && gate.total.available_permits() + 1 == baseline.1
                {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap_or(false);
        let connection_free =
            match tokio::time::timeout(Duration::from_millis(500), pool.acquire()).await {
                Ok(Ok(connection)) => {
                    drop(connection);
                    true
                }
                _ => false,
            };
        let mut probe = f.admin.begin().await.unwrap();
        let source_unlocked = query(&format!(
            "SELECT version FROM {}.collaboration_auth_source_schema FOR UPDATE NOWAIT",
            f.schema
        ))
        .execute(&mut *probe)
        .await
        .is_ok();
        probe.rollback().await.unwrap();
        pending.abort();
        let cancelled = pending.await.is_err_and(|error| error.is_cancelled());
        let retained = gate.workers.available_permits() + 1 == baseline.0
            && gate.total.available_permits() + 1 == baseline.1;
        drop(release);
        let blocker_released = blocker.await.unwrap();
        let recovered = tokio::time::timeout(Duration::from_secs(3), async {
            while (
                gate.workers.available_permits(),
                gate.total.available_permits(),
            ) != baseline
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .is_ok();
        let after = f.snapshot().await;
        pool.close().await;
        f.cleanup().await;
        // Even the expected old-code Red reaches cleanup before any behavior assertion.
        assert!(executor_held && blocker_released);
        assert!(
            dispatched,
            "missing accounts must still dispatch one bounded password job"
        );
        assert!(
            connection_free,
            "password queue waiting must release the one source connection"
        );
        assert!(
            source_unlocked,
            "password queue waiting must release the source metadata lock"
        );
        assert!(cancelled);
        assert!(
            retained,
            "cancelled waiters must not release an actually queued job's permits"
        );
        assert!(recovered);
        assert_eq!(after, before);
    });
}
