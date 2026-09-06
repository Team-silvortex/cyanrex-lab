use super::{RunnerAcquireError, RunnerConfig, RunnerManager};
use crate::{
    models::ebpf::{
        EbpfAttachmentDetail, EbpfCheckResponse, EbpfCompletionResponse, EbpfRunResponse,
        EbpfRuntimeBackend,
    },
    services::{
        ebpf_loader::EbpfLoader,
        runner_driver::{
            RunnerCheckRequest, RunnerCompilerOutcome, RunnerCompletionRequest,
            RunnerDetachOutcome, RunnerDetachRequest, RunnerDriver, RunnerDriverDescriptor,
            RunnerExecutionFuture, RunnerExecutionRequest, RunnerOperationError,
            RunnerOperationFuture,
        },
    },
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct RecordingDriver {
    calls: Arc<AtomicUsize>,
}

impl RunnerDriver for RecordingDriver {
    fn check<'a>(
        &'a self,
        _: RunnerCheckRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCheckResponse>> {
        Box::pin(async { Err(RunnerOperationError::Unsupported) })
    }
    fn complete<'a>(
        &'a self,
        _: RunnerCompletionRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCompletionResponse>> {
        Box::pin(async { Err(RunnerOperationError::Unsupported) })
    }
    fn descriptor(&self) -> RunnerDriverDescriptor {
        RunnerDriverDescriptor {
            mode: "test_driver",
            isolation: "test_vm",
        }
    }

    fn execute<'a>(&'a self, _request: RunnerExecutionRequest<'a>) -> RunnerExecutionFuture<'a> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { EbpfRunResponse::validation_error("recorded") })
    }

    fn list_attachments<'a>(
        &'a self,
        _: &'a str,
    ) -> RunnerOperationFuture<'a, Vec<EbpfAttachmentDetail>> {
        Box::pin(async { Err(RunnerOperationError::Unsupported) })
    }

    fn detach<'a>(
        &'a self,
        _: RunnerDetachRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerDetachOutcome> {
        Box::pin(async { Err(RunnerOperationError::Unsupported) })
    }
}

fn manager(max_concurrent: usize, max_per_user: usize) -> RunnerManager {
    RunnerManager::local(
        RunnerConfig {
            max_concurrent,
            max_per_user,
            execution_timeout: Duration::from_secs(45),
            instance_id: "test".to_string(),
        },
        EbpfLoader::default(),
    )
}

#[tokio::test]
async fn compiler_operations_require_an_owner_before_dispatch() {
    let manager = manager(1, 1);
    let check = manager
        .check(RunnerCheckRequest {
            owner_username: "",
            code: "int value;",
            selected_headers: &[],
        })
        .await
        .err();
    let completion = manager
        .complete(RunnerCompletionRequest {
            owner_username: " ",
            code: "int value;",
            line: 1,
            column: 5,
            selected_headers: &[],
        })
        .await
        .err();
    for error in [check, completion] {
        assert!(matches!(
            error,
            Some(RunnerOperationError::InvalidRequest(_))
        ));
    }
}

#[test]
fn enforces_per_user_capacity_and_releases_on_drop() {
    let manager = manager(2, 1);
    let first = manager
        .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
        .unwrap();
    assert_eq!(
        manager
            .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
            .err(),
        Some(RunnerAcquireError::UserCapacity)
    );
    assert!(manager.try_acquire("bob", EbpfRuntimeBackend::Aya).is_ok());
    drop(first);
    assert!(manager
        .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
        .is_ok());
}

#[test]
fn reports_global_capacity_without_exposing_other_user_count() {
    let manager = manager(1, 1);
    let _lease = manager
        .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
        .unwrap();
    assert_eq!(
        manager.try_acquire("bob", EbpfRuntimeBackend::Aya).err(),
        Some(RunnerAcquireError::GlobalCapacity)
    );
    let status = manager.status_for("bob");
    assert_eq!(status.active_total, 1);
    assert_eq!(status.active_for_current_user, 0);
    assert_eq!(status.available_slots, 0);
}

#[test]
fn unsupported_driver_mode_fails_closed() {
    let config = RunnerConfig {
        max_concurrent: 2,
        max_per_user: 1,
        execution_timeout: Duration::from_secs(45),
        instance_id: "test".to_string(),
    };
    let error = RunnerManager::from_mode(config, EbpfLoader::default(), "remote_vm")
        .err()
        .expect("unsupported mode should fail");
    assert!(error.contains("supports only `local_process`"));
}

#[tokio::test]
async fn execution_delegates_to_driver_and_updates_runtime_metadata() {
    let calls = Arc::new(AtomicUsize::new(0));
    let manager = RunnerManager::new(
        RunnerConfig {
            max_concurrent: 1,
            max_per_user: 1,
            execution_timeout: Duration::from_secs(45),
            instance_id: "test".to_string(),
        },
        Arc::new(RecordingDriver {
            calls: calls.clone(),
        }),
    );
    let lease = manager
        .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
        .unwrap();
    let response = manager
        .execute(
            &lease,
            RunnerExecutionRequest {
                owner_username: "alice",
                code: "int main(void) { return 0; }",
                program_name: Some("test"),
                runtime_backend: EbpfRuntimeBackend::Aya,
                selected_headers: &[],
                debug_breakpoints: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(response.message, "recorded");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let overview = manager.overview();
    assert_eq!(overview.status.mode, "test_driver");
    assert_eq!(overview.status.isolation, "test_vm");
    assert_eq!(overview.active_leases[0].runtime_backend, "aya");
}

#[tokio::test]
async fn execution_rejects_another_owner_before_calling_driver_or_changing_lease() {
    let calls = Arc::new(AtomicUsize::new(0));
    let manager = RunnerManager::new(
        RunnerConfig {
            max_concurrent: 1,
            max_per_user: 1,
            execution_timeout: Duration::from_secs(45),
            instance_id: "owner-test".to_string(),
        },
        Arc::new(RecordingDriver {
            calls: calls.clone(),
        }),
    );
    let lease = manager
        .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
        .unwrap();
    let result = manager
        .execute(
            &lease,
            RunnerExecutionRequest {
                owner_username: "bob",
                code: "int main(void) { return 0; }",
                program_name: None,
                runtime_backend: EbpfRuntimeBackend::Aya,
                selected_headers: &[],
                debug_breakpoints: None,
            },
        )
        .await;

    assert_eq!(
        result.err(),
        Some(super::RunnerExecutionError::InvalidLease)
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let overview = manager.overview();
    assert_eq!(overview.active_leases[0].username, "alice");
    assert_eq!(overview.active_leases[0].runtime_backend, "bpftool");
    assert_eq!(manager.status_for("alice").active_for_current_user, 1);
    drop(lease);
    assert_eq!(manager.status_for("alice").active_total, 0);
}
