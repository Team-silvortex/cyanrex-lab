impl EbpfLoader {
    fn resolve_selected_include_path(
        include_hint: &str,
        local_path: &str,
        header_id: &str,
    ) -> Result<String, String> {
        let trimmed_hint = include_hint.trim();
        if let Some(start) = trimmed_hint.find('<') {
            let suffix = &trimmed_hint[start + 1..];
            if let Some(end) = suffix.find('>') {
                let include_path = suffix[..end].trim();
                if !include_path.is_empty() {
                    return Ok(include_path.to_string());
                }
            }
        }

        if let Some(start) = trimmed_hint.find('"') {
            let suffix = &trimmed_hint[start + 1..];
            if let Some(end) = suffix.find('"') {
                let include_path = suffix[..end].trim();
                if !include_path.is_empty() {
                    return Ok(include_path.to_string());
                }
            }
        }

        let fallback = Path::new(local_path)
            .file_name()
            .map(|value| value.to_string_lossy().to_string());
        if let Some(value) = fallback {
            if !value.is_empty() {
                return Ok(value);
            }
        }

        Err(format!(
            "header '{header_id}' has no valid include hint ('{include_hint}') and no filename could be inferred from local_path",
        ))
    }

    async fn inject_selected_headers(
        temp_dir: &Path,
        selected_headers: &[SelectedHeaderMetadata],
    ) -> Result<(), String> {
        for selected in selected_headers {
            let include_path = Self::resolve_selected_include_path(
                &selected.include_hint,
                &selected.local_path,
                &selected.id,
            )?;

            if selected.local_path.trim().is_empty() {
                return Err(format!(
                    "selected header '{}' has empty local_path, cannot inject include '{}'",
                    selected.id, include_path
                ));
            }

            let source_path = Path::new(&selected.local_path);
            if !source_path.exists() {
                return Err(format!(
                    "selected header '{}' is not available at {}. Download it in C Header Module first, then retry selecting it.",
                    selected.id, selected.local_path
                ));
            }
            match source_path.is_file() {
                true => {}
                false => {
                    return Err(format!(
                        "selected header '{}' resolves to {} but this path is not a regular file (expected the downloaded header file).",
                        selected.id,
                        source_path.display()
                    ));
                }
            }

            let target_path = temp_dir.join(include_path);
            if let Some(parent) = target_path.parent() {
                if let Err(error) = fs::create_dir_all(parent).await {
                    return Err(format!(
                        "failed to create compiler workspace directory for header '{}': {} (target: {}, source: {})",
                        selected.id,
                        error,
                        target_path.display(),
                        source_path.display()
                    ));
                }
            }

            if target_path.exists() {
                let existing_metadata = match fs::metadata(&target_path).await {
                    Ok(value) => Some(value),
                    Err(error) => {
                        if error.kind() == std::io::ErrorKind::NotFound {
                            None
                        } else {
                            return Err(format!(
                                "failed to inspect existing injected header path '{}' for '{}': {error}",
                                target_path.display(),
                                selected.id
                            ));
                        }
                    }
                };
                if let Some(existing_metadata) = existing_metadata {
                    if existing_metadata.is_dir() {
                        if let Err(error) = fs::remove_dir_all(&target_path).await {
                            return Err(format!(
                                "failed to clear existing injected header directory '{}' for '{}': {error}",
                                target_path.display(),
                                selected.id
                            ));
                        }
                    } else if let Err(error) = fs::remove_file(&target_path).await {
                        return Err(format!(
                            "failed to clear existing injected header file '{}' for '{}': {error}",
                            target_path.display(),
                            selected.id
                        ));
                    }
                }
            }

            #[cfg(unix)]
            match std::os::unix::fs::symlink(source_path, &target_path) {
                Ok(()) => {}
                Err(error) => {
                    let fallback = fs::copy(&source_path, &target_path).await;
                    if let Err(fallback_error) = fallback {
                        return Err(format!(
                            "failed to inject header '{}' ({}) into compiler workspace. symlink error: {error}, fallback copy error: {fallback_error}.",
                            selected.id,
                            source_path.display()
                        ));
                    }
                }
            }

            #[cfg(not(unix))]
            if let Err(error) = fs::copy(source_path, &target_path).await {
                return Err(format!(
                    "failed to copy selected header '{}' ({}) into compiler workspace: {error}",
                    selected.id,
                    source_path.display()
                ));
            }
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[allow(clippy::too_many_arguments)]
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
                debug: None,
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
                debug: None,
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
                    debug: None,
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
                debug: None,
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
                debug: None,
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
                debug: None,
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
            debug: None,
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::too_many_arguments)]
    async fn run_with_aya(
        &self,
        _owner_username: &str,
        _code: &str,
        _program_name: Option<&str>,
        _object_path: &Path,
        _bpffs_pin: &Path,
        compile_stdout: String,
        compile_stderr: String,
    ) -> EbpfRunResponse {
        EbpfRunResponse {
            success: false,
            stage: "load".to_string(),
            message: "aya runtime backend is supported only on Linux".to_string(),
            compile_stdout,
            compile_stderr,
            load_stdout: String::new(),
            load_stderr: "aya backend requires Linux kernel and aya crate bindings".to_string(),
            pin_path: None,
            debug: None,
        }
    }

    fn resolve_multiarch_include() -> Option<PathBuf> {
        MULTIARCH_INCLUDE_CACHE
            .get_or_init(|| {
                [
                    "/usr/include/x86_64-linux-gnu",
                    "/usr/include/aarch64-linux-gnu",
                    "/usr/include/arm-linux-gnueabihf",
                    "/usr/include/riscv64-linux-gnu",
                ]
                .iter()
                .map(PathBuf::from)
                .find(|dir| dir.join("asm/types.h").exists())
            })
            .clone()
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
        let cached = VMLINUX_HEADER_CACHE
            .get_or_try_init(|| async {
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
                    return Err(format!(
                        "bpftool btf dump failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
                let cache_path = std::env::temp_dir().join(format!(
                    "cyanrex-vmlinux-{}-{}.h",
                    std::process::id(),
                    Uuid::new_v4()
                ));
                fs::write(&cache_path, output.stdout)
                    .await
                    .map_err(|err| format!("failed to cache generated vmlinux.h: {err}"))?;
                Ok(cache_path)
            })
            .await?;
        let header_path = temp_dir.join("vmlinux.h");
        #[cfg(unix)]
        std::os::unix::fs::symlink(cached, &header_path)
            .map_err(|err| format!("failed to link cached vmlinux.h: {err}"))?;
        #[cfg(not(unix))]
        fs::copy(cached, &header_path)
            .await
            .map_err(|err| format!("failed to copy cached vmlinux.h: {err}"))?;

        Ok(())
    }
}
