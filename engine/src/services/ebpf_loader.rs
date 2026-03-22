use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::{fs, process::Command};

use crate::models::ebpf::EbpfRunResponse;

#[derive(Clone, Default)]
pub struct EbpfLoader;

impl EbpfLoader {
    pub async fn run(&self, code: &str) -> EbpfRunResponse {
        if code.trim().is_empty() {
            return EbpfRunResponse::validation_error("eBPF source code is empty");
        }

        let temp_dir = std::env::temp_dir().join(format!(
            "cyanrex-ebpf-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));

        if let Err(err) = fs::create_dir_all(&temp_dir).await {
            return EbpfRunResponse {
                success: false,
                stage: "setup".to_string(),
                message: format!("failed to create temp directory: {err}"),
                compile_stdout: String::new(),
                compile_stderr: String::new(),
                load_stdout: String::new(),
                load_stderr: String::new(),
            };
        }

        let source_path = temp_dir.join("program.c");
        let object_path = temp_dir.join("program.o");

        if let Err(err) = fs::write(&source_path, code).await {
            return EbpfRunResponse {
                success: false,
                stage: "setup".to_string(),
                message: format!("failed to write source file: {err}"),
                compile_stdout: String::new(),
                compile_stderr: String::new(),
                load_stdout: String::new(),
                load_stderr: String::new(),
            };
        }

        if code.contains("\"vmlinux.h\"") {
            if let Err(err) = Self::ensure_vmlinux_header(&temp_dir).await {
                return EbpfRunResponse {
                    success: false,
                    stage: "compile".to_string(),
                    message: format!("failed to prepare vmlinux.h: {err}"),
                    compile_stdout: String::new(),
                    compile_stderr: String::new(),
                    load_stdout: String::new(),
                    load_stderr: String::new(),
                };
            }
        }

        let clang_bin = Self::resolve_clang_binary();
        let mut compile_cmd = Command::new(clang_bin);
        compile_cmd
            .arg("-O2")
            .arg("-g")
            .arg("-target")
            .arg("bpf")
            .arg("-I")
            .arg("/usr/include")
            .arg("-I")
            .arg(&temp_dir)
            .arg("-c")
            .arg(&source_path)
            .arg("-o")
            .arg(&object_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(multiarch_include) = Self::resolve_multiarch_include() {
            compile_cmd.arg("-I").arg(multiarch_include);
        }

        let compile = compile_cmd.output().await;

        let compile = match compile {
            Ok(output) => output,
            Err(err) => {
                return EbpfRunResponse {
                    success: false,
                    stage: "compile".to_string(),
                    message: format!("failed to execute clang: {err}"),
                    compile_stdout: String::new(),
                    compile_stderr: String::new(),
                    load_stdout: String::new(),
                    load_stderr: String::new(),
                }
            }
        };

        let compile_stdout = String::from_utf8_lossy(&compile.stdout).to_string();
        let compile_stderr = String::from_utf8_lossy(&compile.stderr).to_string();

        if !compile.status.success() {
            return EbpfRunResponse {
                success: false,
                stage: "compile".to_string(),
                message: "clang failed to compile eBPF source".to_string(),
                compile_stdout,
                compile_stderr,
                load_stdout: String::new(),
                load_stderr: String::new(),
            };
        }

        let bpffs_pin = Self::pin_path();

        let load = Command::new("bpftool")
            .arg("prog")
            .arg("loadall")
            .arg(&object_path)
            .arg(&bpffs_pin)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        let load = match load {
            Ok(output) => output,
            Err(err) => {
                return EbpfRunResponse {
                    success: false,
                    stage: "load".to_string(),
                    message: format!("failed to execute bpftool: {err}"),
                    compile_stdout,
                    compile_stderr,
                    load_stdout: String::new(),
                    load_stderr: String::new(),
                }
            }
        };

        let load_stdout = String::from_utf8_lossy(&load.stdout).to_string();
        let load_stderr = String::from_utf8_lossy(&load.stderr).to_string();

        if !load.status.success() {
            return EbpfRunResponse {
                success: false,
                stage: "load".to_string(),
                message: "bpftool failed to load eBPF object".to_string(),
                compile_stdout,
                compile_stderr,
                load_stdout,
                load_stderr,
            };
        }

        EbpfRunResponse {
            success: true,
            stage: "run".to_string(),
            message: "eBPF code compiled and loaded successfully".to_string(),
            compile_stdout,
            compile_stderr,
            load_stdout,
            load_stderr,
        }
    }

    fn pin_path() -> PathBuf {
        let name = format!(
            "/sys/fs/bpf/cyanrex_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        );
        PathBuf::from(name)
    }

    fn resolve_clang_binary() -> &'static str {
        if Path::new("/usr/bin/clang").exists() {
            "/usr/bin/clang"
        } else {
            "clang"
        }
    }

    fn resolve_multiarch_include() -> Option<PathBuf> {
        let candidates = [
            "/usr/include/x86_64-linux-gnu",
            "/usr/include/aarch64-linux-gnu",
            "/usr/include/arm-linux-gnueabihf",
            "/usr/include/riscv64-linux-gnu",
        ];

        candidates
            .iter()
            .map(PathBuf::from)
            .find(|dir| dir.join("asm/types.h").exists())
    }

    async fn ensure_vmlinux_header(temp_dir: &Path) -> Result<(), String> {
        let btf_path = Path::new("/sys/kernel/btf/vmlinux");
        if !btf_path.exists() {
            return Err("kernel BTF file /sys/kernel/btf/vmlinux not found".to_string());
        }

        let output = Command::new("bpftool")
            .arg("btf")
            .arg("dump")
            .arg("file")
            .arg(btf_path)
            .arg("format")
            .arg("c")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|err| format!("failed to execute bpftool btf dump: {err}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(format!("bpftool btf dump failed: {stderr}"));
        }

        let header_path = temp_dir.join("vmlinux.h");
        fs::write(&header_path, output.stdout)
            .await
            .map_err(|err| format!("failed to write generated vmlinux.h: {err}"))?;

        Ok(())
    }
}
