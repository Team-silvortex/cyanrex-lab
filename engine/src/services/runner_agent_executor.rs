use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};
use uuid::Uuid;

use crate::models::runner_job::{RunnerJobClaim, RunnerJobResultState};

const MAX_SOURCE_BYTES: usize = 256 * 1024;
const MAX_OBJECT_BYTES: u64 = 8 * 1024 * 1024;
const STREAM_CAPTURE_BYTES: usize = 3 * 1024;

#[derive(Clone)]
pub struct RunnerCompileExecutorConfig {
    compiler_path: PathBuf,
    work_root: PathBuf,
}

pub struct RunnerJobExecution {
    pub state: RunnerJobResultState,
    pub message: String,
    pub output: Option<String>,
}

#[derive(Serialize)]
struct CompileReport {
    success: bool,
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stdout_truncated: bool,
    stderr: String,
    stderr_truncated: bool,
    object_bytes: Option<u64>,
    object_sha256: Option<String>,
    duration_ms: u128,
}

struct CapturedStream {
    text: String,
    truncated: bool,
}

struct CompileWorkspace {
    path: PathBuf,
}

impl RunnerCompileExecutorConfig {
    pub fn from_env() -> Result<Self, String> {
        let compiler_path = std::env::var("CYANREX_AGENT_CLANG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/usr/bin/clang"));
        let work_root = std::env::var("CYANREX_AGENT_COMPILE_WORK_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("cyanrex-runner-agent"));
        Self::new(compiler_path, work_root)
    }

    pub fn new(compiler_path: PathBuf, work_root: PathBuf) -> Result<Self, String> {
        if !compiler_path.is_absolute()
            || !compiler_path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value == "clang" || value.starts_with("clang-"))
        {
            return Err("clang path must be an absolute clang executable path".to_string());
        }
        if !compiler_path.is_file() {
            return Err(format!(
                "clang executable was not found at {}",
                compiler_path.display()
            ));
        }
        if !work_root.is_absolute() || work_root.parent().is_none() {
            return Err("compile work directory must be an absolute non-root path".to_string());
        }
        if !cfg!(target_os = "linux") {
            return Err("remote compile checking is supported only on Linux or WSL2".to_string());
        }
        Ok(Self {
            compiler_path,
            work_root,
        })
    }
}

pub async fn execute_runner_job(
    agent_id: &str,
    job: &RunnerJobClaim,
    compile_config: Option<&RunnerCompileExecutorConfig>,
) -> RunnerJobExecution {
    match job.kind.as_str() {
        "control_probe" => RunnerJobExecution {
            state: RunnerJobResultState::Succeeded,
            message: "control probe completed".to_string(),
            output: Some(
                serde_json::json!({
                    "agent_id": agent_id,
                    "echo": job.message,
                    "kernel_release": kernel_release(),
                    "arch": std::env::consts::ARCH,
                    "observed_at": Utc::now(),
                })
                .to_string(),
            ),
        },
        "ebpf_compile_check" => execute_compile_job(job, compile_config).await,
        _ => RunnerJobExecution {
            state: RunnerJobResultState::Failed,
            message: "unsupported Runner job kind".to_string(),
            output: None,
        },
    }
}

async fn execute_compile_job(
    job: &RunnerJobClaim,
    config: Option<&RunnerCompileExecutorConfig>,
) -> RunnerJobExecution {
    let Some(config) = config else {
        return failed("remote compile checking is disabled", None);
    };
    let Some(source) = job.source.as_deref() else {
        return failed("compile-check job has no source", None);
    };
    if let Err(error) = validate_compile_source(source) {
        return failed(error, None);
    }
    let remaining_ms = job
        .deadline
        .signed_duration_since(Utc::now())
        .num_milliseconds();
    if remaining_ms <= 0 {
        return failed("compile-check deadline already expired", None);
    }
    match compile_source(config, source, Duration::from_millis(remaining_ms as u64)).await {
        Ok(report) => {
            let state = if report.success {
                RunnerJobResultState::Succeeded
            } else {
                RunnerJobResultState::Failed
            };
            let message = if report.success {
                "remote eBPF compile check passed"
            } else if report.timed_out {
                "remote eBPF compile check timed out"
            } else {
                "remote eBPF compile check failed"
            };
            RunnerJobExecution {
                state,
                message: message.to_string(),
                output: serde_json::to_string(&report).ok(),
            }
        }
        Err(error) => failed(
            "remote eBPF compile executor failed",
            Some(serde_json::json!({"executor_error": error}).to_string()),
        ),
    }
}

pub(crate) fn validate_compile_source(source: &str) -> Result<(), &'static str> {
    if source.trim().is_empty()
        || source.len() > MAX_SOURCE_BYTES
        || source
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err("compile-check source failed text validation");
    }
    let normalized = source
        .replace("\\\r\n", "")
        .replace("\\\n", "")
        .replace("\\\r", "");
    if normalized.contains("__has_include")
        || normalized.contains("__has_embed")
        || normalized.contains(".incbin")
        || normalized.contains("%:")
        || normalized.contains("??=")
        || normalized
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .any(|token| matches!(token, "asm" | "__asm" | "__asm__"))
    {
        return Err("compile-check source uses a blocked file lookup directive");
    }
    for line in normalized.lines() {
        let Some(directive) = line.trim_start().strip_prefix('#').map(str::trim_start) else {
            continue;
        };
        if directive.starts_with("include_next")
            || directive.starts_with("import")
            || directive.starts_with("embed")
        {
            return Err("compile-check source uses an unsupported include directive");
        }
        let Some(argument) = directive.strip_prefix("include") else {
            continue;
        };
        if !argument
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_whitespace() || character == '<')
        {
            continue;
        }
        let argument = argument.trim_start();
        if !safe_system_include(argument) {
            return Err("compile-check includes must be literal safe system headers");
        }
    }
    Ok(())
}

fn safe_system_include(argument: &str) -> bool {
    let Some(header) = argument
        .strip_prefix('<')
        .and_then(|value| value.split_once('>').map(|(header, _)| header))
    else {
        return false;
    };
    !header.is_empty()
        && header.len() <= 128
        && !header.starts_with('/')
        && !header.split('/').any(|component| component == "..")
        && header.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '/')
        })
}

async fn compile_source(
    config: &RunnerCompileExecutorConfig,
    source: &str,
    available: Duration,
) -> Result<CompileReport, String> {
    let workspace = CompileWorkspace::create(&config.work_root)?;
    let source_path = workspace.path.join("program.c");
    let object_path = workspace.path.join("program.o");
    write_private_file(&source_path, source.as_bytes())?;

    let allowed = available.min(Duration::from_secs(60));
    let started = Instant::now();
    let mut command = Command::new(&config.compiler_path);
    command
        .current_dir(&workspace.path)
        .env_clear()
        .env("LANG", "C")
        .arg("-target")
        .arg("bpf")
        .arg("-O2")
        .arg("-g")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-fno-color-diagnostics")
        .arg("-I/usr/include")
        .arg("-c")
        .arg("program.c")
        .arg("-o")
        .arg("program.o")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    add_multiarch_include(&mut command);
    add_target_arch_define(&mut command);
    configure_resource_limits(&mut command, allowed);

    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or("clang stdout pipe is unavailable")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("clang stderr pipe is unavailable")?;
    let stdout_task = tokio::spawn(read_capped(stdout, STREAM_CAPTURE_BYTES));
    let stderr_task = tokio::spawn(read_capped(stderr, STREAM_CAPTURE_BYTES));
    let (status, timed_out) = match timeout(allowed, child.wait()).await {
        Ok(Ok(status)) => (Some(status), false),
        Ok(Err(error)) => return Err(format!("cannot wait for clang: {error}")),
        Err(_) => {
            stop_compile_process(&mut child).await?;
            (None, true)
        }
    };
    let stdout = stdout_task
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let stderr = stderr_task
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let success = status.as_ref().is_some_and(|value| value.success());
    let (object_bytes, object_sha256) = if success {
        object_summary(&object_path)?
    } else {
        (None, None)
    };
    Ok(CompileReport {
        success,
        exit_code: status.and_then(|value| value.code()),
        timed_out,
        stdout: stdout.text,
        stdout_truncated: stdout.truncated,
        stderr: stderr.text,
        stderr_truncated: stderr.truncated,
        object_bytes,
        object_sha256,
        duration_ms: started.elapsed().as_millis(),
    })
}

impl CompileWorkspace {
    fn create(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|error| error.to_string())?;
        let metadata = fs::symlink_metadata(root).map_err(|error| error.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("compile work root must be a real directory".to_string());
        }
        set_private_directory_permissions(root)?;
        let path = root.join(format!("job-{}", Uuid::new_v4().simple()));
        create_private_directory(&path)?;
        Ok(Self { path })
    }
}

impl Drop for CompileWorkspace {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            if error.kind() != io::ErrorKind::NotFound {
                tracing::warn!(path = %self.path.display(), %error, "compile workspace cleanup failed");
            }
        }
    }
}

async fn read_capped<R: AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
) -> io::Result<CapturedStream> {
    let mut kept = Vec::with_capacity(limit);
    let mut total = 0usize;
    let mut buffer = [0u8; 4096];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count);
        let remaining = limit.saturating_sub(kept.len());
        kept.extend_from_slice(&buffer[..count.min(remaining)]);
    }
    Ok(CapturedStream {
        text: String::from_utf8_lossy(&kept).into_owned(),
        truncated: total > kept.len(),
    })
}

fn object_summary(path: &Path) -> Result<(Option<u64>, Option<String>), String> {
    let size = fs::metadata(path).map_err(|error| error.to_string())?.len();
    if size > MAX_OBJECT_BYTES {
        return Err("compiled object exceeds the 8 MiB limit".to_string());
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    Ok((Some(size), Some(format!("{:x}", Sha256::digest(bytes)))))
}

fn add_multiarch_include(command: &mut Command) {
    let candidate = match std::env::consts::ARCH {
        "x86_64" => Some("/usr/include/x86_64-linux-gnu"),
        "aarch64" => Some("/usr/include/aarch64-linux-gnu"),
        _ => None,
    };
    if candidate.is_some_and(|path| Path::new(path).is_dir()) {
        command.arg(format!("-I{}", candidate.unwrap()));
    }
}

fn add_target_arch_define(command: &mut Command) {
    match std::env::consts::ARCH {
        "x86_64" => {
            command.arg("-D__TARGET_ARCH_x86");
        }
        "aarch64" => {
            command.arg("-D__TARGET_ARCH_arm64");
        }
        _ => {}
    }
}

fn failed(message: &str, output: Option<String>) -> RunnerJobExecution {
    RunnerJobExecution {
        state: RunnerJobResultState::Failed,
        message: message.to_string(),
        output,
    }
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| error.to_string())
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(path).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir(path).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn configure_resource_limits(command: &mut Command, allowed: Duration) {
    let cpu_seconds = allowed.as_secs().saturating_add(1).min(61);
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            set_limit(libc::RLIMIT_CPU, cpu_seconds)?;
            set_limit(libc::RLIMIT_FSIZE, MAX_OBJECT_BYTES)?;
            set_limit(libc::RLIMIT_NOFILE, 64)?;
            set_limit(libc::RLIMIT_CORE, 0)?;
            set_limit(libc::RLIMIT_AS, 1024 * 1024 * 1024)
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_resource_limits(_command: &mut Command, _allowed: Duration) {}

#[cfg(target_os = "linux")]
type RlimitResource = libc::__rlimit_resource_t;

#[cfg(target_os = "linux")]
fn set_limit(resource: RlimitResource, desired: u64) -> io::Result<()> {
    let mut current = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(resource, &mut current) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let bounded = (desired as libc::rlim_t).min(current.rlim_max);
    let limit = libc::rlimit {
        rlim_cur: bounded,
        rlim_max: bounded,
    };
    if unsafe { libc::setrlimit(resource, &limit) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
async fn stop_compile_process(child: &mut tokio::process::Child) -> Result<(), String> {
    let Some(pid) = child.id() else {
        return Ok(());
    };
    let result = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
    if result != 0 && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
        child
            .kill()
            .await
            .map_err(|error| format!("cannot stop timed-out clang: {error}"))?;
    } else {
        child
            .wait()
            .await
            .map_err(|error| format!("cannot reap timed-out clang: {error}"))?;
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
async fn stop_compile_process(child: &mut tokio::process::Child) -> Result<(), String> {
    child
        .kill()
        .await
        .map_err(|error| format!("cannot stop timed-out clang: {error}"))
}

pub(crate) fn kernel_release() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|value| value.trim().chars().take(128).collect())
        .filter(|value: &String| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("runner_agent_executor/tests.inc.rs");
}
