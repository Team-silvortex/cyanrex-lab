use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;
use tokio::sync::oneshot;

// Dropping an assertion scope must release every test worker before its runtime shuts down.
struct Release(Option<mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}
fn held_work() -> (
    Release,
    oneshot::Receiver<()>,
    impl FnOnce() -> usize + Send + 'static,
) {
    let (release, wait) = mpsc::channel();
    let (entered, started) = oneshot::channel();
    (Release(Some(release)), started, move || {
        let _ = entered.send(());
        wait.recv_timeout(Duration::from_secs(5))
            .expect("test worker was not released");
        17
    })
}
async fn started(receiver: oneshot::Receiver<()>) {
    tokio::time::timeout(Duration::from_secs(2), receiver)
        .await
        .expect("worker did not reach its explicit barrier")
        .unwrap();
}
async fn permits(gate: &PasswordWorkGate, workers: usize, total: usize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if gate.workers.available_permits() == workers
                && gate.total.available_permits() == total
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("password-work permit lifecycle did not settle");
}

#[tokio::test]
async fn running_job_keeps_both_permits_after_its_waiter_is_cancelled() {
    let gate = Arc::new(PasswordWorkGate::new(1, 1));
    let (release, entered, work) = held_work();
    let runner = gate.clone();
    let waiting = tokio::spawn(async move { runner.run(|| work).await });
    started(entered).await;
    waiting.abort();
    assert!(waiting.await.unwrap_err().is_cancelled());
    assert_eq!(gate.run(|| || 99).await, Err(DurableAuthError::RateLimited));
    assert_eq!(gate.workers.available_permits(), 0);
    assert_eq!(gate.total.available_permits(), 0);
    drop(release);
    permits(&gate, 1, 1).await;
    assert_eq!(gate.run(|| || 23).await, Ok(23));
}

#[tokio::test]
async fn four_dispatched_and_sixteen_waiting_jobs_exhaust_the_twenty_job_budget() {
    let gate = Arc::new(PasswordWorkGate::new(4, 20));
    let (entered, mut arrivals) = tokio::sync::mpsc::unbounded_channel();
    let prepared = Arc::new(AtomicUsize::new(0));
    let mut releases = Vec::new();
    let mut waiting = Vec::new();
    for index in 0..20 {
        let (release, wait) = mpsc::channel();
        releases.push(Release(Some(release)));
        let gate = gate.clone();
        let entered = entered.clone();
        let prepared = prepared.clone();
        waiting.push(tokio::spawn(async move {
            gate.run(|| {
                prepared.fetch_add(1, Ordering::SeqCst);
                move || {
                    entered.send(index).unwrap();
                    wait.recv_timeout(Duration::from_secs(5))
                        .expect("saturated test worker was not released");
                    index
                }
            })
            .await
        }));
    }
    permits(&gate, 0, 0).await;
    for _ in 0..4 {
        tokio::time::timeout(Duration::from_secs(2), arrivals.recv())
            .await
            .unwrap()
            .unwrap();
    }
    assert!(
        arrivals.try_recv().is_err(),
        "more than four workers were dispatched"
    );
    assert_eq!(
        prepared.load(Ordering::SeqCst),
        4,
        "queued jobs must not clone secret inputs before worker admission"
    );
    assert_eq!(gate.run(|| || 21).await, Err(DurableAuthError::RateLimited));
    drop(releases);
    for (index, job) in waiting.into_iter().enumerate() {
        assert_eq!(job.await.unwrap(), Ok(index));
    }
    permits(&gate, 4, 20).await;
}

#[tokio::test]
async fn an_unpolled_request_does_not_prepare_or_admit_work() {
    let gate = PasswordWorkGate::new(1, 2);
    let prepared = AtomicBool::new(false);
    let request = gate.run(|| {
        prepared.store(true, Ordering::SeqCst);
        || 1
    });
    drop(request);
    assert!(!prepared.load(Ordering::SeqCst));
    permits(&gate, 1, 2).await;
}

#[tokio::test]
async fn cancelling_a_worker_slot_waiter_returns_only_its_total_admission() {
    let gate = Arc::new(PasswordWorkGate::new(1, 2));
    let (release, entered, work) = held_work();
    let first_gate = gate.clone();
    let first = tokio::spawn(async move { first_gate.run(|| work).await });
    started(entered).await;
    let prepared = Arc::new(AtomicBool::new(false));
    let second_prepared = prepared.clone();
    let second_gate = gate.clone();
    let second = tokio::spawn(async move {
        second_gate
            .run(|| {
                second_prepared.store(true, Ordering::SeqCst);
                || 99
            })
            .await
    });
    permits(&gate, 0, 0).await;
    second.abort();
    assert!(second.await.unwrap_err().is_cancelled());
    permits(&gate, 0, 1).await;
    assert!(!prepared.load(Ordering::SeqCst));
    drop(release);
    assert_eq!(first.await.unwrap(), Ok(17));
    permits(&gate, 1, 2).await;
}

#[tokio::test]
async fn a_zero_worker_gate_can_time_out_without_preparing_or_leaking_admission() {
    let gate = PasswordWorkGate::new(0, 1);
    let prepared = AtomicBool::new(false);
    let request = gate.run(|| {
        prepared.store(true, Ordering::SeqCst);
        || 1
    });
    assert!(tokio::time::timeout(Duration::from_millis(30), request)
        .await
        .is_err());
    assert!(!prepared.load(Ordering::SeqCst));
    permits(&gate, 0, 1).await;
}

#[tokio::test]
async fn timing_out_a_running_request_keeps_its_work_admitted_until_real_completion() {
    let gate = Arc::new(PasswordWorkGate::new(1, 1));
    let (release, entered, work) = held_work();
    let runner = gate.clone();
    let waiting = tokio::spawn(async move {
        let operation = runner.run(|| work);
        tokio::pin!(operation);
        tokio::select! {
            _ = &mut operation => panic!("held work completed before release"),
            signal = entered => signal.unwrap(),
        }
        tokio::time::timeout(Duration::from_millis(30), operation).await
    });
    assert!(waiting.await.unwrap().is_err());
    assert_eq!(gate.run(|| || 99).await, Err(DurableAuthError::RateLimited));
    permits(&gate, 0, 0).await;
    drop(release);
    permits(&gate, 1, 1).await;
}

#[test]
fn cancelled_work_in_tokios_blocking_queue_keeps_both_permits_until_it_runs() {
    // A separate runtime makes the blocking queue deterministic without consuming the
    // production-wide gate or another test's runtime threads.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (external_release, external_entered, external_work) = held_work();
        let external = tokio::task::spawn_blocking(external_work);
        started(external_entered).await;
        let gate = Arc::new(PasswordWorkGate::new(1, 1));
        let (release, mut entered, work) = held_work();
        let (prepared, preparation) = oneshot::channel();
        let runner = gate.clone();
        let waiting = tokio::spawn(async move {
            runner
                .run(|| {
                    prepared.send(()).unwrap();
                    work
                })
                .await
        });
        started(preparation).await;
        waiting.abort();
        assert!(waiting.await.unwrap_err().is_cancelled());
        assert!(matches!(
            entered.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        assert_eq!(gate.run(|| || 99).await, Err(DurableAuthError::RateLimited));
        permits(&gate, 0, 0).await;
        drop(external_release);
        assert_eq!(external.await.unwrap(), 17);
        started(entered).await;
        assert_eq!(gate.run(|| || 99).await, Err(DurableAuthError::RateLimited));
        drop(release);
        permits(&gate, 1, 1).await;
        assert_eq!(gate.run(|| || 23).await, Ok(23));
    });
}

#[tokio::test]
async fn a_panicking_worker_is_storage_failure_and_returns_both_permits() {
    let gate = PasswordWorkGate::new(1, 1);
    let failed = gate
        .run(|| || -> usize { panic!("synthetic password worker failure") })
        .await;
    assert_eq!(failed, Err(DurableAuthError::StorageUnavailable));
    permits(&gate, 1, 1).await;
    assert_eq!(gate.run(|| || 23).await, Ok(23));
}

#[tokio::test]
async fn closed_admission_or_worker_gates_reject_without_preparing_secrets() {
    for close_total in [true, false] {
        let gate = PasswordWorkGate::new(1, 1);
        let prepared = AtomicBool::new(false);
        if close_total {
            gate.total.close();
        } else {
            gate.workers.close();
        }
        let result = gate
            .run(|| {
                prepared.store(true, Ordering::SeqCst);
                || 1
            })
            .await;
        assert_eq!(result, Err(DurableAuthError::StorageUnavailable));
        assert!(!prepared.load(Ordering::SeqCst));
        permits(&gate, 1, 1).await;
    }
}

#[test]
fn independent_callers_share_one_process_gate_across_threads() {
    let address = shared_gate() as *const PasswordWorkGate as usize;
    let other_address = std::thread::spawn(|| shared_gate() as *const PasswordWorkGate as usize)
        .join()
        .unwrap();
    assert_eq!(address, other_address);
    assert_eq!(address, shared_gate() as *const PasswordWorkGate as usize);
}

#[tokio::test]
async fn real_password_work_retains_argon2_and_wrong_password_semantics() {
    let password = "synthetic-password-worker-correct-密码";
    let salt = "00000000-0000-4000-8000-000000000001";
    let encoded = derive_password_hash(password, salt).await.unwrap();
    assert!(encoded.starts_with("$argon2"));
    assert_eq!(verify_password(password, salt, &encoded).await, Ok(true));
    assert_eq!(
        verify_password(password, "unrelated-salt-column", &encoded).await,
        Ok(true)
    );
    assert_eq!(
        verify_password("wrong-password", salt, &encoded).await,
        Ok(false)
    );
    assert_eq!(
        verify_password(password, salt, "$argon2-invalid").await,
        Err(DurableAuthError::InvalidRecord)
    );
}

#[tokio::test]
async fn an_unsupported_low_cost_phc_is_invalid_record_not_a_password_result() {
    use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, PasswordHasher, Version};

    // This regression is safe against the old unbounded verifier: its only PHC has
    // an intentionally tiny cost. Hostile/high-cost profiles belong only in pure tests.
    let password = "synthetic-low-cost-profile-password";
    let salt = SaltString::encode_b64(b"synthetic-low-cost-salt").unwrap();
    let encoded = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(8, 1, 1, Some(32)).unwrap(),
    )
    .hash_password(password.as_bytes(), &salt)
    .unwrap()
    .to_string();
    assert_eq!(
        verify_password(password, "unrelated-salt-column", &encoded).await,
        Err(DurableAuthError::InvalidRecord)
    );
}
