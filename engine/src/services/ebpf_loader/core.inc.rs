impl EbpfLoader {
    pub async fn run(
        &self,
        owner_username: &str,
        code: &str,
        program_name: Option<&str>,
        runtime_backend: EbpfRuntimeBackend,
        selected_headers: &[SelectedHeaderMetadata],
        _debug_breakpoints: Option<&[u32]>,
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

        if let Err(err) = Self::inject_selected_headers(&temp_dir, selected_headers).await {
            return EbpfRunResponse {
                success: false,
                stage: "compile".to_string(),
                message: format!("failed to prepare selected headers: {err}"),
                compile_stdout: String::new(),
                compile_stderr: String::new(),
                load_stdout: String::new(),
                load_stderr: String::new(),
                pin_path: None,
            };
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
        if runtime_backend != EbpfRuntimeBackend::Aya {
            if let Err(error) = fs::create_dir_all(&bpffs_pin).await {
                return EbpfRunResponse {
                    success: false,
                    stage: "setup".to_string(),
                    message: format!("failed to prepare pin directory: {error}"),
                    compile_stdout,
                    compile_stderr,
                    load_stdout: String::new(),
                    load_stderr: String::new(),
                    pin_path: Some(bpffs_pin.display().to_string()),
                };
            }
        }

        if runtime_backend == EbpfRuntimeBackend::Aya {
            return self
                .run_with_aya(
                    owner_username,
                    code,
                    program_name,
                    &object_path,
                    &bpffs_pin,
                    compile_stdout,
                    compile_stderr,
                )
                .await;
        }

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
                String::from_utf8_lossy(&fallback.stdout)
            );
            load_stderr = format!(
                "{load_stderr}\n{}",
                String::from_utf8_lossy(&fallback.stderr)
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
            let is_aya_path = {
                let sessions = self.aya_sessions.read().await;
                sessions.contains_key(&path)
            };

            if is_aya_path {
                let mut sessions = self.aya_sessions.write().await;
                sessions.remove(&path);
                let _ = fs::remove_dir_all(&path).await;
            } else {
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

    #[cfg(target_os = "linux")]
    pub async fn poll_aya_ringbuf(
        &self,
        pin_path: &str,
        preferred_map_name: &str,
        max_items: usize,
    ) -> Result<Vec<Vec<u8>>, String> {
        let mut sessions = self.aya_sessions.write().await;
        let session = sessions
            .get_mut(pin_path)
            .ok_or_else(|| "aya session not found for pin path".to_string())?;

        let mut map_name = None;
        if session._ebpf.map(preferred_map_name).is_some() {
            map_name = Some(preferred_map_name.to_string());
        }
        if map_name.is_none() {
            for (name, map) in session._ebpf.maps() {
                if matches!(map, aya::maps::Map::RingBuf(_)) {
                    map_name = Some(name.to_string());
                    break;
                }
            }
        }

        let map_name = map_name.ok_or_else(|| "no ringbuf map found in aya session".to_string())?;
        let map = session
            ._ebpf
            .map_mut(&map_name)
            .ok_or_else(|| format!("ringbuf map not found: {map_name}"))?;
        let mut ring =
            RingBuf::try_from(map).map_err(|err| format!("failed to open aya ringbuf: {err}"))?;

        let mut out = Vec::new();
        for _ in 0..max_items {
            let Some(item) = ring.next() else {
                break;
            };
            out.push(item.to_vec());
        }

        Ok(out)
    }

    #[cfg(not(target_os = "linux"))]
    pub async fn poll_aya_ringbuf(
        &self,
        _pin_path: &str,
        _preferred_map_name: &str,
        _max_items: usize,
    ) -> Result<Vec<Vec<u8>>, String> {
        Err("aya runtime backend is supported only on Linux".to_string())
    }

    fn pin_path() -> PathBuf {
        let namespace = crate::config::runtime_instance_id();
        let name = format!("{}_{}", std::process::id(), chrono::Utc::now().timestamp_millis());
        PathBuf::from("/sys/fs/bpf/cyanrex")
            .join(namespace)
            .join(name)
    }

    fn validate_pin_path(path: &str) -> Result<(), String> {
        if Path::new(path)
            .components()
            .any(|component| component == std::path::Component::ParentDir)
        {
            return Err("pin path contains parent directory traversal".to_string());
        }

        let namespace = Path::new("/sys/fs/bpf/cyanrex").join(crate::config::runtime_instance_id());
        if !Path::new(path).starts_with(namespace) {
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
}
