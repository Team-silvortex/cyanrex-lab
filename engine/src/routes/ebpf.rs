use std::{
    path::Path,
    sync::{Arc, OnceLock},
};

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde_json::{json, Value};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    sync::Semaphore,
    time::{Duration, Instant},
};

use crate::{
    models::{
        ebpf::{
            EbpfAttachmentDetail, EbpfAttachmentDetailListResponse, EbpfAttachmentListResponse,
            EbpfCheckBackend, EbpfCheckBackendInventory, EbpfCheckResponse, EbpfCompletionRequest,
            EbpfCompletionResponse, EbpfDetachRequest, EbpfDetachResponse,
            EbpfRemoteCheckCancelRequest, EbpfRemoteCheckResponse, EbpfRemoteCheckStatusQuery,
            EbpfRemoteCheckSubmitRequest, EbpfRunRequest, EbpfRunResponse, EbpfRuntimeBackend,
            EbpfTemplate,
        },
        event::{Event, EventCategory, EventSeverity},
        runner_agent::{RunnerAgentIsolation, RunnerAgentState},
        runner_job::{RunnerCompileReport, RunnerJobState, RunnerJobView},
    },
    services::{
        runner_driver::RunnerExecutionRequest, runner_job_queue::RunnerJobQueueError,
        runner_manager::RunnerExecutionError,
    },
    AppState,
};

const MAX_EBPF_SOURCE_BYTES: usize = 256 * 1024;
static EBPF_CHECK_SLOTS: OnceLock<Semaphore> = OnceLock::new();
static EBPF_COMPLETION_SLOTS: OnceLock<Semaphore> = OnceLock::new();

include!("ebpf/handlers.inc.rs");
include!("ebpf/learning.inc.rs");
include!("ebpf/check.inc.rs");
include!("ebpf/remote_check.inc.rs");
include!("ebpf/completion.inc.rs");
include!("ebpf/stream.inc.rs");
include!("ebpf/ringbuf.inc.rs");
include!("ebpf/templates.inc.rs");
