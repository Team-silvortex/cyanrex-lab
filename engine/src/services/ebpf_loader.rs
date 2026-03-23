use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use tokio::{fs, process::Command, sync::RwLock};

use crate::models::ebpf::EbpfRunResponse;

#[derive(Clone, Default)]
pub struct EbpfLoader {
    attachments: Arc<RwLock<BTreeMap<String, AttachmentRecord>>>,
}

#[derive(Clone, Debug)]
struct AttachmentRecord {
    owner_username: String,
    source: String,
    program_name: String,
}

impl EbpfLoader {
    pub async fn run(
        &self,
        owner_username: &str,
        code: &str,
        program_name: Option<&str>,
    ) -> EbpfRunResponse {
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
                pin_path: None,
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
                pin_path: None,
            };
        }

        if Self::requires_vmlinux_header(code) {
            if let Err(err) = Self::ensure_vmlinux_header(&temp_dir).await {
                return EbpfRunResponse {
                    success: false,
                    stage: "compile".to_string(),
                    message: format!("failed to prepare vmlinux.h: {err}"),
                    compile_stdout: String::new(),
                    compile_stderr: String::new(),
                    load_stdout: String::new(),
                    load_stderr: String::new(),
                    pin_path: None,
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
                    pin_path: None,
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
                pin_path: None,
            };
        }

        let bpffs_pin = Self::pin_path();

        let load_with_attach = Command::new("bpftool")
            .arg("prog")
            .arg("loadall")
            .arg(&object_path)
            .arg(&bpffs_pin)
            .arg("autoattach")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        let load_with_attach = match load_with_attach {
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
                    pin_path: Some(bpffs_pin.display().to_string()),
                }
            }
        };

        let mut load_stdout = String::from_utf8_lossy(&load_with_attach.stdout).to_string();
        let mut load_stderr = String::from_utf8_lossy(&load_with_attach.stderr).to_string();
        let mut attach_enabled = load_with_attach.status.success();
        let mut attach_mode = if attach_enabled {
            "autoattach".to_string()
        } else {
            "none".to_string()
        };

        if !load_with_attach.status.success() && Self::autoattach_unsupported(&load_stderr) {
            let fallback = Command::new("bpftool")
                .arg("prog")
                .arg("loadall")
                .arg(&object_path)
                .arg(&bpffs_pin)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;

            let fallback = match fallback {
                Ok(output) => output,
                Err(err) => {
                    return EbpfRunResponse {
                        success: false,
                        stage: "load".to_string(),
                        message: format!("failed to execute bpftool fallback load: {err}"),
                        compile_stdout,
                        compile_stderr,
                        load_stdout,
                        load_stderr,
                        pin_path: Some(bpffs_pin.display().to_string()),
                    };
                }
            };

            attach_enabled = false;
            load_stdout = format!(
                "{load_stdout}\n{}",
                String::from_utf8_lossy(&fallback.stdout).to_string()
            );
            load_stderr = format!(
                "{load_stderr}\n{}",
                String::from_utf8_lossy(&fallback.stderr).to_string()
            );

            if !fallback.status.success() {
                return EbpfRunResponse {
                    success: false,
                    stage: "load".to_string(),
                    message: "bpftool failed to load eBPF object".to_string(),
                    compile_stdout,
                    compile_stderr,
                    load_stdout,
                    load_stderr,
                    pin_path: Some(bpffs_pin.display().to_string()),
                };
            }

            if let Ok((attached, attach_log)) =
                Self::manual_attach_tracepoints(&bpffs_pin, code).await
            {
                if !attach_log.is_empty() {
                    load_stderr = format!("{load_stderr}\n{attach_log}");
                }
                if attached {
                    attach_enabled = true;
                    attach_mode = "manual-tracepoint".to_string();
                }
            }
        } else if !load_with_attach.status.success() {
            return EbpfRunResponse {
                success: false,
                stage: "load".to_string(),
                message: "bpftool failed to load eBPF object".to_string(),
                compile_stdout,
                compile_stderr,
                load_stdout,
                load_stderr,
                pin_path: Some(bpffs_pin.display().to_string()),
            };
        }

        let pin_path = bpffs_pin.display().to_string();
        let mut attachments = self.attachments.write().await;
        attachments.insert(
            pin_path.clone(),
            AttachmentRecord {
                owner_username: owner_username.to_string(),
                source: code.to_string(),
                program_name: program_name
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .unwrap_or("custom")
                    .to_string(),
            },
        );

        EbpfRunResponse {
            success: true,
            stage: "run".to_string(),
            message: if attach_enabled {
                if attach_mode == "manual-tracepoint" {
                    "eBPF code compiled, loaded, and manually attached successfully".to_string()
                } else {
                    "eBPF code compiled, loaded, and auto-attached successfully".to_string()
                }
            } else {
                "eBPF code compiled and loaded successfully (autoattach unsupported by bpftool)"
                    .to_string()
            },
            compile_stdout,
            compile_stderr,
            load_stdout,
            load_stderr,
            pin_path: Some(pin_path),
        }
    }

    pub async fn detach_for_user(
        &self,
        username: &str,
        pin_path: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let targets = if let Some(path) = pin_path {
            let attachments = self.attachments.read().await;
            let Some(record) = attachments.get(path) else {
                return Err("pin path is not tracked by cyanrex".to_string());
            };
            if record.owner_username != username {
                return Err("pin path belongs to another user".to_string());
            }
            vec![path.to_string()]
        } else {
            self.list_attachments_for_user(username).await
        };

        let mut detached = Vec::new();
        for path in targets {
            Self::validate_pin_path(&path)?;
            let metadata = fs::metadata(&path)
                .await
                .map_err(|err| format!("failed to stat pin path {path}: {err}"))?;

            if metadata.is_dir() {
                fs::remove_dir_all(&path)
                    .await
                    .map_err(|err| format!("failed to remove pin directory {path}: {err}"))?;
            } else {
                fs::remove_file(&path)
                    .await
                    .map_err(|err| format!("failed to remove pin file {path}: {err}"))?;
            }

            detached.push(path.clone());
        }

        if !detached.is_empty() {
            let mut attachments = self.attachments.write().await;
            for path in &detached {
                attachments.remove(path);
            }
        }

        Ok(detached)
    }

    pub async fn list_attachments_for_user(&self, username: &str) -> Vec<String> {
        let attachments = self.attachments.read().await;
        attachments
            .iter()
            .filter_map(|(pin_path, record)| {
                if record.owner_username == username {
                    Some(pin_path.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn list_attachment_details_for_user(
        &self,
        username: &str,
    ) -> Vec<(String, String, String)> {
        let attachments = self.attachments.read().await;
        attachments
            .iter()
            .filter_map(|(pin_path, record)| {
                if record.owner_username != username {
                    return None;
                }
                Some((
                    pin_path.clone(),
                    record.source.clone(),
                    record.program_name.clone(),
                ))
            })
            .collect()
    }

    fn pin_path() -> PathBuf {
        let name = format!(
            "/sys/fs/bpf/cyanrex_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        );
        PathBuf::from(name)
    }

    fn validate_pin_path(path: &str) -> Result<(), String> {
        if !path.starts_with("/sys/fs/bpf/cyanrex_") {
            return Err("pin path is outside cyanrex managed namespace".to_string());
        }
        Ok(())
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

    fn autoattach_unsupported(stderr: &str) -> bool {
        let text = stderr.to_ascii_lowercase();
        text.contains("autoattach")
            && (text.contains("unknown")
                || text.contains("invalid")
                || text.contains("unrecognized")
                || text.contains("expected"))
    }

    fn requires_vmlinux_header(code: &str) -> bool {
        code.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("#include")
                && (trimmed.contains("<vmlinux.h>") || trimmed.contains("\"vmlinux.h\""))
        })
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

    async fn manual_attach_tracepoints(
        pin_root: &Path,
        code: &str,
    ) -> Result<(bool, String), String> {
        let sections = Self::extract_tracepoint_sections(code);
        if sections.is_empty() {
            return Ok((
                false,
                "manual attach skipped: no tracepoint SEC found".to_string(),
            ));
        }

        let prog_paths = Self::list_pinned_prog_paths(pin_root).await?;
        if prog_paths.is_empty() {
            return Ok((
                false,
                "manual attach skipped: no pinned programs found".to_string(),
            ));
        }

        let mut logs = Vec::new();
        let mut any_success = false;

        for (category, name) in sections {
            let target = format!("{category}:{name}");
            let mut section_attached = false;

            for prog in &prog_paths {
                let attempts = [
                    vec![
                        "prog".to_string(),
                        "attach".to_string(),
                        "pinned".to_string(),
                        prog.clone(),
                        "tracepoint".to_string(),
                        target.clone(),
                    ],
                    vec![
                        "prog".to_string(),
                        "attach".to_string(),
                        "pinned".to_string(),
                        prog.clone(),
                        "tp".to_string(),
                        target.clone(),
                    ],
                ];

                for args in attempts {
                    let output = Command::new("bpftool")
                        .args(args.iter().map(String::as_str))
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                        .await
                        .map_err(|error| {
                            format!("failed to execute bpftool manual attach: {error}")
                        })?;

                    if output.status.success() {
                        logs.push(format!("manual attach success: {prog} -> {target}"));
                        section_attached = true;
                        any_success = true;
                        break;
                    }

                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if !stderr.is_empty() {
                        logs.push(format!(
                            "manual attach attempt failed ({prog} -> {target}): {stderr}"
                        ));
                    }
                }

                if section_attached {
                    break;
                }
            }

            if !section_attached {
                logs.push(format!(
                    "manual attach failed for tracepoint target {target}"
                ));
            }
        }

        Ok((any_success, logs.join("\n")))
    }

    fn extract_tracepoint_sections(code: &str) -> Vec<(String, String)> {
        let mut sections = Vec::new();
        for line in code.lines() {
            let trimmed = line.trim();
            if !trimmed.contains("SEC(\"tracepoint/") {
                continue;
            }
            let Some(start_idx) = trimmed.find("SEC(\"tracepoint/") else {
                continue;
            };
            let segment = &trimmed[start_idx + "SEC(\"tracepoint/".len()..];
            let Some(end_quote) = segment.find('"') else {
                continue;
            };
            let raw = &segment[..end_quote];
            let mut parts = raw.splitn(2, '/');
            let Some(category) = parts.next() else {
                continue;
            };
            let Some(name) = parts.next() else {
                continue;
            };
            if !category.is_empty() && !name.is_empty() {
                sections.push((category.to_string(), name.to_string()));
            }
        }
        sections
    }

    async fn list_pinned_prog_paths(pin_root: &Path) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut entries = fs::read_dir(pin_root).await.map_err(|error| {
            format!("failed to list pinned dir {}: {error}", pin_root.display())
        })?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| format!("failed to read pinned dir entry: {error}"))?
        {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            if file_name == "maps" {
                continue;
            }
            out.push(path.display().to_string());
        }
        Ok(out)
    }
}
