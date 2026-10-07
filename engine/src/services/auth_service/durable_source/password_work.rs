//! Process-wide admission for explicit durable-source password work only, not live AuthService.
use super::{password_profile, DurableAuthError, Result};
use std::sync::{Arc, OnceLock};
use tokio::sync::{Semaphore, TryAcquireError};

pub(super) struct PasswordWorkGate {
    workers: Arc<Semaphore>,
    total: Arc<Semaphore>,
}
impl PasswordWorkGate {
    fn new(workers: usize, total: usize) -> Self {
        Self {
            workers: Arc::new(Semaphore::new(workers)),
            total: Arc::new(Semaphore::new(total)),
        }
    }

    async fn run<T, F, P>(&self, prepare: P) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
        P: FnOnce() -> F + Send,
    {
        let admitted = self
            .total
            .clone()
            .try_acquire_owned()
            .map_err(|error| match error {
                TryAcquireError::NoPermits => DurableAuthError::RateLimited,
                TryAcquireError::Closed => DurableAuthError::StorageUnavailable,
            })?;
        // Waiting is part of the caller's existing deadline. Cancellation here drops admission
        // and never clones secrets or dispatches work into Tokio's blocking queue.
        let worker = self
            .workers
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let work = prepare();
        tokio::task::spawn_blocking(move || {
            // Both permits belong to the job, including time queued in Tokio. Dropping/timing out
            // its JoinHandle must not advertise capacity while that job is still alive.
            let _permits = (admitted, worker);
            work()
        })
        .await
        .map_err(|_| DurableAuthError::StorageUnavailable)
    }
}

fn shared_gate() -> &'static PasswordWorkGate {
    static GATE: OnceLock<PasswordWorkGate> = OnceLock::new();
    GATE.get_or_init(|| PasswordWorkGate::new(4, 20))
}

pub(super) async fn derive_password_hash(password: &str, salt: &str) -> Result<String> {
    shared_gate()
        .run(|| {
            let password = password.to_owned();
            let salt = salt.to_owned();
            move || password_profile::derive(&password, &salt)
        })
        .await?
}

pub(super) async fn verify_password(
    password: &str,
    _salt: &str,
    expected_hash: &str,
) -> Result<bool> {
    // The PHC carries its own salt. The separate legacy salt column is not a second authority.
    // Reject malformed/unsupported records before admission or secret copies.
    password_profile::validate(expected_hash)?;
    shared_gate()
        .run(|| {
            let password = password.to_owned();
            let expected_hash = expected_hash.to_owned();
            move || password_profile::verify(&password, &expected_hash)
        })
        .await?
}

#[cfg(test)]
#[path = "password_work_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "login_password_sql_tests.rs"]
mod login_sql_tests;
