use std::{path::Path, sync::Arc};

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
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
            EbpfAttachmentDetail, EbpfAttachmentDetailListResponse, EbpfAttachmentListResponse,
            EbpfDetachRequest, EbpfDetachResponse, EbpfRunRequest, EbpfRunResponse,
            EbpfRuntimeBackend, EbpfTemplate,
        },
        event::{Event, EventCategory, EventSeverity},
    },
    AppState,
};

const MAX_EBPF_SOURCE_BYTES: usize = 256 * 1024;

include!("ebpf/handlers.inc.rs");
include!("ebpf/stream.inc.rs");
include!("ebpf/ringbuf.inc.rs");
include!("ebpf/templates.inc.rs");
