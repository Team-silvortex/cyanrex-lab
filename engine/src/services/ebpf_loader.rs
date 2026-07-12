use std::{
    collections::BTreeMap,
    convert::TryFrom,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use aya::{maps::RingBuf, programs::TracePoint, Ebpf};
use tokio::{fs, process::Command, sync::RwLock, time::Duration};
use uuid::Uuid;

use crate::models::ebpf::{
    EbpfCheckResponse, EbpfCompilerDiagnostic, EbpfCompletionItem, EbpfCompletionResponse,
    EbpfRunResponse, EbpfRuntimeBackend,
};

#[derive(Clone, Default)]
pub struct EbpfLoader {
    attachments: Arc<RwLock<BTreeMap<String, AttachmentRecord>>>,
    aya_sessions: Arc<RwLock<BTreeMap<String, AyaSession>>>,
}

#[derive(Clone, Debug)]
struct AttachmentRecord {
    owner_username: String,
    source: String,
    program_name: String,
}

struct AyaSession {
    _ebpf: Ebpf,
}

include!("ebpf_loader/core.inc.rs");
include!("ebpf_loader/check.inc.rs");
include!("ebpf_loader/completion.inc.rs");
include!("ebpf_loader/aya.inc.rs");
include!("ebpf_loader/attach.inc.rs");
