use std::{future::Future, pin::Pin};

use crate::{
    models::{
        c_headers::SelectedHeaderMetadata,
        ebpf::{
            EbpfAttachmentDetail, EbpfCheckResponse, EbpfCompletionResponse, EbpfRunResponse,
            EbpfRuntimeBackend,
        },
    },
    services::ebpf_loader::EbpfLoader,
};

pub type RunnerExecutionFuture<'a> = Pin<Box<dyn Future<Output = EbpfRunResponse> + Send + 'a>>;
pub type RunnerOperationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RunnerOperationError>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerOperationError {
    #[error("compiler is busy; retry shortly")]
    Busy,
    #[error("{0}")]
    InvalidRequest(String),
    #[error("runner execution environment is unavailable")]
    Unavailable,
    #[error("runner driver does not support this operation")]
    Unsupported,
    #[error("runner operation exceeded its deadline")]
    Timeout,
}

pub struct RunnerDetachRequest<'a> {
    pub owner_username: &'a str,
    pub pin_path: Option<&'a str>,
}

pub struct RunnerCheckRequest<'a> {
    pub owner_username: &'a str,
    pub code: &'a str,
    pub selected_headers: &'a [SelectedHeaderMetadata],
}

pub struct RunnerCompletionRequest<'a> {
    pub owner_username: &'a str,
    pub code: &'a str,
    pub line: usize,
    pub column: usize,
    pub selected_headers: &'a [SelectedHeaderMetadata],
}

pub struct RunnerCompilerOutcome<T> {
    pub response: T,
    // None means the backend did not report cache use; it must not be counted as a cache miss.
    pub cache_hit: Option<bool>,
}

#[derive(Debug)]
pub struct RunnerDetachOutcome {
    pub detached: Vec<String>,
    pub clean: bool,
    pub safety_notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerDriverDescriptor {
    pub mode: &'static str,
    pub isolation: &'static str,
}

pub struct RunnerExecutionRequest<'a> {
    pub owner_username: &'a str,
    pub code: &'a str,
    pub program_name: Option<&'a str>,
    pub runtime_backend: EbpfRuntimeBackend,
    pub selected_headers: &'a [SelectedHeaderMetadata],
    pub debug_breakpoints: Option<&'a [u32]>,
}

pub trait RunnerDriver: Send + Sync {
    fn descriptor(&self) -> RunnerDriverDescriptor;

    fn execute<'a>(&'a self, request: RunnerExecutionRequest<'a>) -> RunnerExecutionFuture<'a>;

    fn check<'a>(
        &'a self,
        request: RunnerCheckRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCheckResponse>>;

    fn complete<'a>(
        &'a self,
        request: RunnerCompletionRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCompletionResponse>>;

    // Attachment lifetime is independent of the short-lived execution capacity lease.
    // Remote drivers must enforce ownership and inspect pins in the execution environment,
    // not on the control host. Returning a cleanup report does not certify a VM safe for reuse.
    fn list_attachments<'a>(
        &'a self,
        owner_username: &'a str,
    ) -> RunnerOperationFuture<'a, Vec<EbpfAttachmentDetail>>;

    fn detach<'a>(
        &'a self,
        request: RunnerDetachRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerDetachOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_attachment_lifecycle_preserves_empty_inventory_and_cleanup() {
        let driver = LocalProcessRunnerDriver::new(EbpfLoader::default());
        assert!(driver.list_attachments("alice").await.unwrap().is_empty());
        let outcome = driver
            .detach(RunnerDetachRequest {
                owner_username: "alice",
                pin_path: None,
            })
            .await
            .unwrap();
        assert!(outcome.clean);
        assert!(outcome.detached.is_empty());
        assert!(outcome.safety_notes.is_empty());
    }

    #[tokio::test]
    async fn local_detach_rejects_untracked_paths_without_touching_them() {
        let driver = LocalProcessRunnerDriver::new(EbpfLoader::default());
        let path = std::env::temp_dir().join(format!("cyanrex-untracked-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, "not a tracked attachment")
            .await
            .unwrap();
        let result = driver
            .detach(RunnerDetachRequest {
                owner_username: "alice",
                pin_path: Some(path.to_str().unwrap()),
            })
            .await;
        assert_eq!(
            result.unwrap_err(),
            RunnerOperationError::InvalidRequest("pin path is not tracked by cyanrex".into())
        );
        assert_eq!(
            tokio::fs::read_to_string(&path).await.unwrap(),
            "not a tracked attachment"
        );
        tokio::fs::remove_file(path).await.unwrap();
    }
}

#[derive(Clone)]
pub struct LocalProcessRunnerDriver {
    loader: EbpfLoader,
}

impl LocalProcessRunnerDriver {
    pub fn new(loader: EbpfLoader) -> Self {
        Self { loader }
    }
}

impl RunnerDriver for LocalProcessRunnerDriver {
    fn descriptor(&self) -> RunnerDriverDescriptor {
        RunnerDriverDescriptor {
            mode: "local_process",
            isolation: "shared_kernel",
        }
    }

    fn execute<'a>(&'a self, request: RunnerExecutionRequest<'a>) -> RunnerExecutionFuture<'a> {
        Box::pin(async move {
            self.loader
                .run(
                    request.owner_username,
                    request.code,
                    request.program_name,
                    request.runtime_backend,
                    request.selected_headers,
                    request.debug_breakpoints,
                )
                .await
        })
    }

    fn check<'a>(
        &'a self,
        request: RunnerCheckRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCheckResponse>> {
        Box::pin(async move {
            let (response, cache_hit) = self
                .loader
                .check_for_user_with_cache_status(
                    request.owner_username,
                    request.code,
                    request.selected_headers,
                )
                .await;
            Ok(RunnerCompilerOutcome {
                response,
                cache_hit: Some(cache_hit),
            })
        })
    }

    fn complete<'a>(
        &'a self,
        request: RunnerCompletionRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCompletionResponse>> {
        Box::pin(async move {
            let (response, cache_hit) = self
                .loader
                .complete_for_user_with_cache_status(
                    request.owner_username,
                    request.code,
                    request.line,
                    request.column,
                    request.selected_headers,
                )
                .await;
            Ok(RunnerCompilerOutcome {
                response,
                cache_hit: Some(cache_hit),
            })
        })
    }

    fn list_attachments<'a>(
        &'a self,
        owner_username: &'a str,
    ) -> RunnerOperationFuture<'a, Vec<EbpfAttachmentDetail>> {
        Box::pin(async move {
            Ok(self
                .loader
                .list_attachment_details_for_user(owner_username)
                .await
                .into_iter()
                .map(|(pin_path, source, program_name)| EbpfAttachmentDetail {
                    pin_path,
                    source,
                    program_name,
                })
                .collect())
        })
    }

    fn detach<'a>(
        &'a self,
        request: RunnerDetachRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerDetachOutcome> {
        Box::pin(async move {
            let detached = self
                .loader
                .detach_for_user(request.owner_username, request.pin_path)
                .await
                .map_err(RunnerOperationError::InvalidRequest)?;
            let mut safety_notes = Vec::new();
            for path in &detached {
                match tokio::fs::metadata(path).await {
                    Ok(_) => {
                        safety_notes.push(format!("pin path still exists after detach: {path}"))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        safety_notes.push(format!("cannot verify pin removal for {path}: {error}"))
                    }
                }
            }
            let remaining = self
                .loader
                .list_attachments_for_user(request.owner_username)
                .await;
            for path in &detached {
                if remaining.contains(path) {
                    safety_notes.push(format!("pin path still tracked in attachment set: {path}"));
                }
            }
            if request.pin_path.is_none() && !remaining.is_empty() {
                safety_notes.push(format!(
                    "detach all requested but {} attachment(s) remain",
                    remaining.len()
                ));
            }
            Ok(RunnerDetachOutcome {
                detached,
                clean: safety_notes.is_empty(),
                safety_notes,
            })
        })
    }
}
