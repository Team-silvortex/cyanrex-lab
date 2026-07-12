impl EbpfLoader {
    async fn run_with_aya(
        &self,
        owner_username: &str,
        code: &str,
        program_name: Option<&str>,
        object_path: &Path,
        bpffs_pin: &Path,
        compile_stdout: String,
        compile_stderr: String,
    ) -> EbpfRunResponse {
        let sections = Self::extract_tracepoint_sections(code);
        if sections.is_empty() {
            return EbpfRunResponse {
                success: false,
                stage: "load".to_string(),
                message: "aya backend currently supports tracepoint programs only".to_string(),
                compile_stdout,
                compile_stderr,
                load_stdout: String::new(),
                load_stderr: "no tracepoint SEC(\"tracepoint/... \") found".to_string(),
                pin_path: None,
            };
        }

        let trace_id_path = format!(
            "/sys/kernel/tracing/events/{}/{}/id",
            sections[0].0, sections[0].1
        );
        if !Path::new(&trace_id_path).exists() {
            return EbpfRunResponse {
                success: false,
                stage: "load".to_string(),
                message: "aya tracepoint attach requires tracefs mount".to_string(),
                compile_stdout,
                compile_stderr,
                load_stdout: String::new(),
                load_stderr: format!(
                    "missing tracepoint id path: {} (mount /sys/kernel/tracing into engine container)",
                    trace_id_path
                ),
                pin_path: None,
            };
        }

        let mut ebpf = match Ebpf::load_file(object_path) {
            Ok(value) => value,
            Err(err) => {
                return EbpfRunResponse {
                    success: false,
                    stage: "load".to_string(),
                    message: "aya failed to load eBPF object".to_string(),
                    compile_stdout,
                    compile_stderr,
                    load_stdout: String::new(),
                    load_stderr: format!("aya load_file error: {err}"),
                    pin_path: None,
                };
            }
        };

        let (category, name) = sections[0].clone();
        let mut load_logs = Vec::new();
        let mut attached = false;

        let object_program_keys: Vec<String> =
            ebpf.programs().map(|(name, _)| name.to_string()).collect();
        let mut candidates = Self::extract_function_names(code);
        for key in &object_program_keys {
            if !candidates.iter().any(|existing| existing == key) {
                candidates.push(key.clone());
            }
        }
        load_logs.push(format!("aya tracepoint target: {category}:{name}"));
        load_logs.push(format!(
            "aya object programs: {}",
            object_program_keys.join(", ")
        ));
        load_logs.push(format!("aya attach candidates: {}", candidates.join(", ")));

        for candidate_name in candidates {
            let Some(program) = ebpf.program_mut(&candidate_name) else {
                continue;
            };

            let Ok(tracepoint) = <&mut TracePoint>::try_from(program) else {
                load_logs.push(format!("aya skip non-tracepoint program: {candidate_name}"));
                continue;
            };

            if let Err(err) = tracepoint.load() {
                load_logs.push(format!("aya load failed ({candidate_name}): {err}"));
                continue;
            }

            match tracepoint.attach(&category, &name) {
                Ok(_) => {
                    load_logs.push(format!(
                        "aya attach success: {candidate_name} -> {category}:{name}"
                    ));
                    attached = true;
                    break;
                }
                Err(err) => {
                    load_logs.push(format!(
                        "aya attach failed ({candidate_name} -> {category}:{name}): {err}"
                    ));
                }
            }
        }

        if !attached {
            return EbpfRunResponse {
                success: false,
                stage: "load".to_string(),
                message: "aya failed to attach tracepoint program".to_string(),
                compile_stdout,
                compile_stderr,
                load_stdout: String::new(),
                load_stderr: load_logs.join("\n"),
                pin_path: None,
            };
        }

        if let Err(err) = fs::create_dir_all(bpffs_pin).await {
            return EbpfRunResponse {
                success: false,
                stage: "load".to_string(),
                message: format!("aya attached but failed to create pin directory: {err}"),
                compile_stdout,
                compile_stderr,
                load_stdout: String::new(),
                load_stderr: load_logs.join("\n"),
                pin_path: None,
            };
        }
        let maps_dir = bpffs_pin.join("maps");
        if let Err(err) = fs::create_dir_all(&maps_dir).await {
            return EbpfRunResponse {
                success: false,
                stage: "load".to_string(),
                message: format!("aya attached but failed to create map pin directory: {err}"),
                compile_stdout,
                compile_stderr,
                load_stdout: String::new(),
                load_stderr: load_logs.join("\n"),
                pin_path: None,
            };
        }

        for (name, map) in ebpf.maps() {
            let map_pin = maps_dir.join(name);
            match map.pin(&map_pin) {
                Ok(_) => {
                    load_logs.push(format!("aya map pinned: {} -> {}", name, map_pin.display()))
                }
                Err(err) => load_logs.push(format!("aya map pin failed: {} ({err})", name)),
            }
        }

        let pin_path = bpffs_pin.display().to_string();
        {
            let mut sessions = self.aya_sessions.write().await;
            sessions.insert(pin_path.clone(), AyaSession { _ebpf: ebpf });
        }

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
            message: "eBPF code compiled, loaded, and attached successfully (aya backend)"
                .to_string(),
            compile_stdout,
            compile_stderr,
            load_stdout: load_logs
                .iter()
                .filter(|line| !line.to_ascii_lowercase().contains("failed"))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n"),
            load_stderr: load_logs
                .iter()
                .filter(|line| line.to_ascii_lowercase().contains("failed"))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n"),
            pin_path: Some(pin_path),
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
}
