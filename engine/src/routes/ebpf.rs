use std::{path::Path, sync::Arc};

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
    time::{Duration, Instant},
};

use crate::{
    models::{
        ebpf::{
            EbpfAttachmentDetailListResponse, EbpfAttachmentListResponse, EbpfCheckBackend,
            EbpfCheckBackendInventory, EbpfCheckResponse, EbpfCompletionRequest,
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
        runner_driver::{
            RunnerCheckRequest, RunnerCompletionRequest, RunnerDetachRequest,
            RunnerExecutionRequest, RunnerOperationError,
        },
        runner_job_queue::RunnerJobQueueError,
        runner_manager::RunnerExecutionError,
    },
    AppState,
};

const MAX_EBPF_SOURCE_BYTES: usize = 256 * 1024;

fn runner_operation_status(error: &RunnerOperationError) -> StatusCode {
    match error {
        RunnerOperationError::Busy => StatusCode::TOO_MANY_REQUESTS,
        RunnerOperationError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
        RunnerOperationError::Unavailable | RunnerOperationError::Unsupported => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        RunnerOperationError::Timeout => StatusCode::REQUEST_TIMEOUT,
    }
}

include!("ebpf/handlers.inc.rs");
include!("ebpf/attachments.inc.rs");
include!("ebpf/learning.inc.rs");
include!("ebpf/compiler.inc.rs");
include!("ebpf/check.inc.rs");
include!("ebpf/remote_check.inc.rs");
include!("ebpf/completion.inc.rs");
include!("ebpf/stream.inc.rs");
include!("ebpf/ringbuf.inc.rs");
include!("ebpf/templates.inc.rs");
