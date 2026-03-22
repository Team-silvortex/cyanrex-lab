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
        let memlock = Self::check_memlock().await;

        let checks = vec![clang, bpftool, kernel, btf, bpffs, memlock];
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
}
