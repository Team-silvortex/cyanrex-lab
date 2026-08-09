use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Instant,
};

#[cfg(target_os = "linux")]
use aya::{maps::RingBuf, programs::TracePoint, Ebpf};

use sha2::{Digest, Sha256};
use tokio::{
    fs,
    process::Command,
    sync::{OnceCell, RwLock},
    time::Duration,
};
use uuid::Uuid;

use crate::models::c_headers::SelectedHeaderMetadata;
use crate::models::ebpf::{
    EbpfCheckResponse, EbpfCompilerDiagnostic, EbpfCompletionItem, EbpfCompletionResponse,
    EbpfDebugInfo, EbpfDebugRejectedBreakpoint, EbpfRunResponse, EbpfRuntimeBackend,
};

#[derive(Clone, Default)]
pub struct EbpfLoader {
    attachments: Arc<RwLock<HashMap<String, AttachmentRecord>>>,
    aya_sessions: Arc<RwLock<HashMap<String, AyaSession>>>,
    check_cache: Arc<RwLock<HashMap<String, (Instant, EbpfCheckResponse)>>>,
    completion_cache: Arc<RwLock<HashMap<String, (Instant, EbpfCompletionResponse)>>>,
    resident_compiler: Arc<AtomicBool>,
}

#[derive(Clone, Debug)]
struct AttachmentRecord {
    owner_username: String,
    source: String,
    program_name: String,
}

struct RunWorkspace {
    path: PathBuf,
}

impl RunWorkspace {
    fn new(owner_username: &str) -> Self {
        let path = std::env::temp_dir()
            .join("cyanrex")
            .join(crate::config::runtime_instance_id())
            .join(owner_namespace(owner_username))
            .join(Uuid::new_v4().simple().to_string());
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RunWorkspace {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    path = %self.path.display(),
                    "failed to clean eBPF runner workspace: {error}"
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
struct AyaSession {
    _ebpf: Ebpf,
}

#[cfg(not(target_os = "linux"))]
struct AyaSession;

static VMLINUX_HEADER_CACHE: OnceCell<PathBuf> = OnceCell::const_new();
static MULTIARCH_INCLUDE_CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();

fn source_cache_key(code: &str) -> String {
    format!("{:x}", Sha256::digest(code.as_bytes()))
}

fn owner_namespace(username: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(username.as_bytes()));
    digest[..16].to_string()
}

fn child_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    command
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
include!("ebpf_loader/debug.inc.rs");
include!("ebpf_loader/check.inc.rs");
include!("ebpf_loader/completion.inc.rs");
include!("ebpf_loader/aya.inc.rs");
include!("ebpf_loader/attach.inc.rs");

#[cfg(test)]
mod workspace_tests {
    use super::{owner_namespace, RunWorkspace};

    #[test]
    fn owner_namespace_is_stable_distinct_and_path_safe() {
        let alice = owner_namespace("alice@example.com");
        assert_eq!(alice, owner_namespace("alice@example.com"));
        assert_ne!(alice, owner_namespace("bob@example.com"));
        assert_eq!(alice.len(), 16);
        assert!(alice.chars().all(|value| value.is_ascii_hexdigit()));
    }

    #[test]
    fn workspace_is_removed_when_lease_scope_ends() {
        let path = {
            let workspace = RunWorkspace::new("cleanup-student");
            std::fs::create_dir_all(workspace.path()).unwrap();
            std::fs::write(workspace.path().join("program.c"), "int main(void) {}").unwrap();
            workspace.path().to_path_buf()
        };
        assert!(
            !path.exists(),
            "workspace was not cleaned: {}",
            path.display()
        );
    }
}
