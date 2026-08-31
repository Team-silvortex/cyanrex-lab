use std::{future::Future, pin::Pin};

use crate::{
    models::{
        c_headers::SelectedHeaderMetadata,
        ebpf::{EbpfRunResponse, EbpfRuntimeBackend},
    },
    services::ebpf_loader::EbpfLoader,
};

pub type RunnerExecutionFuture<'a> = Pin<Box<dyn Future<Output = EbpfRunResponse> + Send + 'a>>;

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
}
