use std::{
    collections::BTreeMap,
    convert::TryFrom,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use aya::{maps::RingBuf, programs::TracePoint, Ebpf};
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    process::Command,
    sync::{OnceCell, RwLock},
    time::Duration,
};
use uuid::Uuid;

use crate::models::ebpf::{
    EbpfCheckResponse, EbpfCompilerDiagnostic, EbpfCompletionItem, EbpfCompletionResponse,
    EbpfRunResponse, EbpfRuntimeBackend,
};

#[derive(Clone, Default)]
pub struct EbpfLoader {
    attachments: Arc<RwLock<BTreeMap<String, AttachmentRecord>>>,
    aya_sessions: Arc<RwLock<BTreeMap<String, AyaSession>>>,
    check_cache: Arc<RwLock<BTreeMap<String, (Instant, EbpfCheckResponse)>>>,
    completion_cache: Arc<RwLock<BTreeMap<String, (Instant, EbpfCompletionResponse)>>>,
    resident_compiler: Arc<AtomicBool>,
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

static VMLINUX_HEADER_CACHE: OnceCell<PathBuf> = OnceCell::const_new();

fn source_cache_key(code: &str) -> String {
    format!("{:x}", Sha256::digest(code.as_bytes()))
}

impl EbpfLoader {
    pub fn resident_compiler_enabled(&self) -> bool {
        self.resident_compiler.load(Ordering::Relaxed)
    }

    pub async fn set_resident_compiler(&self, enabled: bool) {
        self.resident_compiler.store(enabled, Ordering::Relaxed);
        if !enabled {
            self.check_cache.write().await.clear();
            self.completion_cache.write().await.clear();
        }
    }
}

include!("ebpf_loader/core.inc.rs");
include!("ebpf_loader/check.inc.rs");
include!("ebpf_loader/completion.inc.rs");
include!("ebpf_loader/aya.inc.rs");
include!("ebpf_loader/attach.inc.rs");
