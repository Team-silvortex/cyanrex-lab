use std::path::Path;

use chrono::Utc;
use tokio::{fs, process::Command};

use crate::models::environment::{EnvironmentCheckItem, EnvironmentReport};

#[derive(Clone, Default)]
pub struct EnvironmentChecker;

impl EnvironmentChecker {
    pub async fn inspect(&self) -> EnvironmentReport {
        let clang = Self::check_command("clang", &["--version"], "clang").await;
        let bpftool = Self::check_command("bpftool", &["version"], "bpftool").await;
        let kernel = Self::check_command("uname", &["-r"], "kernel").await;
        let btf = Self::check_path("kernel_btf", "/sys/kernel/btf/vmlinux").await;
        let bpffs = Self::check_path("bpffs", "/sys/fs/bpf").await;
        let bpffs_mount = Self::check_bpffs_mount().await;
        let btf_dump = Self::check_btf_dump().await;
        let autoattach = Self::check_bpftool_autoattach().await;
        let link_show = Self::check_bpftool_link_show().await;
        let runtime = Self::check_runtime_context().await;
        let memlock = Self::check_memlock().await;

        let checks = vec![
            clang,
            bpftool,
            kernel,
            runtime,
            btf,
            btf_dump,
            bpffs,
            bpffs_mount,
            autoattach,
            link_show,
            memlock,
        ];
        let overall_ok = checks.iter().all(|check| check.ok);

        EnvironmentReport {
            overall_ok,
            generated_at: Utc::now(),
            checks,
        }
    }

    async fn check_command(name: &str, args: &[&str], label: &str) -> EnvironmentCheckItem {
        let output = Command::new(name).args(args).output().await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .lines()
                    .next()
                    .unwrap_or("available")
                    .to_string();
                EnvironmentCheckItem {
                    name: label.to_string(),
                    ok: true,
                    detail: stdout,
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                EnvironmentCheckItem {
                    name: label.to_string(),
                    ok: false,
                    detail: if stderr.is_empty() {
                        format!("{name} command failed")
                    } else {
                        stderr
                    },
                }
            }
            Err(err) => EnvironmentCheckItem {
                name: label.to_string(),
                ok: false,
                detail: format!("{name} not found: {err}"),
            },
        }
    }

    async fn check_path(label: &str, path: &str) -> EnvironmentCheckItem {
        if Path::new(path).exists() {
            EnvironmentCheckItem {
                name: label.to_string(),
                ok: true,
                detail: format!("{path} exists"),
            }
        } else {
            EnvironmentCheckItem {
                name: label.to_string(),
                ok: false,
                detail: format!("{path} missing"),
            }
        }
    }

    async fn check_memlock() -> EnvironmentCheckItem {
        match fs::read_to_string("/proc/self/limits").await {
            Ok(limits) => {
                let line = limits
                    .lines()
                    .find(|line| line.starts_with("Max locked memory"))
                    .unwrap_or("Max locked memory unavailable");

                let ok = line.contains("unlimited");
                EnvironmentCheckItem {
                    name: "memlock".to_string(),
                    ok,
                    detail: line.trim().to_string(),
                }
            }
            Err(err) => EnvironmentCheckItem {
                name: "memlock".to_string(),
                ok: false,
                detail: format!("unable to read /proc/self/limits: {err}"),
            },
        }
    }

    async fn check_bpffs_mount() -> EnvironmentCheckItem {
        match fs::read_to_string("/proc/self/mountinfo").await {
            Ok(content) => {
                let line = content
                    .lines()
                    .find(|line| line.contains(" /sys/fs/bpf ") && line.contains(" - bpf "))
                    .map(str::to_string);

                match line {
                    Some(value) => EnvironmentCheckItem {
                        name: "bpffs_mount_type".to_string(),
                        ok: true,
                        detail: value,
                    },
                    None => EnvironmentCheckItem {
                        name: "bpffs_mount_type".to_string(),
                        ok: false,
                        detail: "/sys/fs/bpf is not mounted as bpffs".to_string(),
                    },
                }
            }
            Err(error) => EnvironmentCheckItem {
                name: "bpffs_mount_type".to_string(),
                ok: false,
                detail: format!("failed to read /proc/self/mountinfo: {error}"),
            },
        }
    }

    async fn check_btf_dump() -> EnvironmentCheckItem {
        let output = Command::new("bpftool")
            .arg("btf")
            .arg("dump")
            .arg("file")
            .arg("/sys/kernel/btf/vmlinux")
            .arg("format")
            .arg("c")
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => EnvironmentCheckItem {
                name: "btf_dump".to_string(),
                ok: true,
                detail: "bpftool can dump /sys/kernel/btf/vmlinux".to_string(),
            },
            Ok(output) => EnvironmentCheckItem {
                name: "btf_dump".to_string(),
                ok: false,
                detail: first_non_empty_line(&output.stderr)
                    .or_else(|| first_non_empty_line(&output.stdout))
                    .unwrap_or_else(|| "bpftool btf dump failed".to_string()),
            },
            Err(error) => EnvironmentCheckItem {
                name: "btf_dump".to_string(),
                ok: false,
                detail: format!("failed to execute bpftool btf dump: {error}"),
            },
        }
    }

    async fn check_bpftool_autoattach() -> EnvironmentCheckItem {
        let output = Command::new("bpftool")
            .arg("prog")
            .arg("help")
            .output()
            .await;

        match output {
            Ok(output) => {
                let combined = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
                .to_ascii_lowercase();
                let supported = combined.contains("autoattach");
                EnvironmentCheckItem {
                    name: "bpftool_autoattach".to_string(),
                    ok: supported,
                    detail: if supported {
                        "supported (bpftool help includes autoattach)".to_string()
                    } else {
                        "not supported by current bpftool".to_string()
                    },
                }
            }
            Err(error) => EnvironmentCheckItem {
                name: "bpftool_autoattach".to_string(),
                ok: false,
                detail: format!("failed to execute bpftool prog help: {error}"),
            },
        }
    }

    async fn check_bpftool_link_show() -> EnvironmentCheckItem {
        let output = Command::new("bpftool")
            .arg("-j")
            .arg("link")
            .arg("show")
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => EnvironmentCheckItem {
                name: "bpftool_link_show".to_string(),
                ok: true,
                detail: "bpftool link show is available".to_string(),
            },
            Ok(output) => EnvironmentCheckItem {
                name: "bpftool_link_show".to_string(),
                ok: false,
                detail: first_non_empty_line(&output.stderr)
                    .or_else(|| first_non_empty_line(&output.stdout))
                    .unwrap_or_else(|| "bpftool link show failed".to_string()),
            },
            Err(error) => EnvironmentCheckItem {
                name: "bpftool_link_show".to_string(),
                ok: false,
                detail: format!("failed to execute bpftool link show: {error}"),
            },
        }
    }

    async fn check_runtime_context() -> EnvironmentCheckItem {
        match fs::read_to_string("/proc/1/cgroup").await {
            Ok(cgroup) => {
                let lower = cgroup.to_ascii_lowercase();
                let detail = if lower.contains("docker")
                    || lower.contains("containerd")
                    || lower.contains("kubepods")
                    || lower.contains("podman")
                {
                    "containerized runtime detected".to_string()
                } else {
                    "host-like runtime context".to_string()
                };
                EnvironmentCheckItem {
                    name: "runtime_context".to_string(),
                    ok: true,
                    detail,
                }
            }
            Err(error) => EnvironmentCheckItem {
                name: "runtime_context".to_string(),
                ok: false,
                detail: format!("failed to read /proc/1/cgroup: {error}"),
            },
        }
    }
}

fn first_non_empty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}
