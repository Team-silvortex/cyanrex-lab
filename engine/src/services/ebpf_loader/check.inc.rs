impl EbpfLoader {
    pub async fn check(&self, code: &str) -> EbpfCheckResponse {
        self.check_with_cache_status(code, &[]).await.0
    }

    pub async fn check_with_cache_status(
        &self,
        code: &str,
        selected_headers: &[SelectedHeaderMetadata],
    ) -> (EbpfCheckResponse, bool) {
        if code.trim().is_empty() {
            return (check_failure("eBPF source code is empty", String::new()), false);
        }

        let cache_key = format!(
            "{}:{}",
            source_cache_key(code),
            selected_headers_cache_key(selected_headers)
        );
        if let Some((created, response)) = self.check_cache.read().await.get(&cache_key) {
            if self.resident_compiler_enabled() || created.elapsed() < Duration::from_secs(60) {
                return (response.clone(), true);
            }
        }

        let temp_dir = std::env::temp_dir().join(format!("cyanrex-check-{}", Uuid::new_v4()));
        if let Err(error) = fs::create_dir(&temp_dir).await {
            return (
                check_failure(&format!("failed to create check directory: {error}"), String::new()),
                false,
            );
        }

        let response = self.check_in_directory(code, selected_headers, &temp_dir).await;
        let _ = fs::remove_dir_all(&temp_dir).await;
        let mut cache = self.check_cache.write().await;
        let cache_limit = if self.resident_compiler_enabled() { 512 } else { 64 };
        if cache.len() >= cache_limit && cache_limit > 0 {
            if let Some(oldest_key) = cache.keys().next().cloned() {
                cache.remove(&oldest_key);
            }
        }
        cache.insert(cache_key, (Instant::now(), response.clone()));
        (response, false)
    }

    async fn check_in_directory(
        &self,
        code: &str,
        selected_headers: &[SelectedHeaderMetadata],
        temp_dir: &Path,
    ) -> EbpfCheckResponse {
        let source_path = temp_dir.join("program.c");
        if let Err(error) = fs::write(&source_path, code).await {
            return check_failure(&format!("failed to write source: {error}"), String::new());
        }

        if Self::requires_vmlinux_header(code) {
            if let Err(error) = Self::ensure_vmlinux_header(temp_dir).await {
                return check_failure(&format!("failed to prepare vmlinux.h: {error}"), String::new());
            }
        }
        if let Err(error) = Self::inject_selected_headers(temp_dir, selected_headers).await {
            return check_failure(
                &format!("failed to prepare selected headers: {error}"),
                String::new(),
            );
        }

        let mut command = Command::new(Self::resolve_clang_binary());
        command
            .kill_on_drop(true)
            .arg("-O2")
            .arg("-g")
            .arg("-target")
            .arg("bpf")
            .arg("-fsyntax-only")
            .arg("-I")
            .arg("/usr/include")
            .arg("-I")
            .arg(temp_dir)
            .arg(&source_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(include) = Self::resolve_multiarch_include() {
            command.arg("-I").arg(include);
        }

        let output = match tokio::time::timeout(Duration::from_secs(15), command.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                return check_failure(&format!("failed to execute clang: {error}"), String::new())
            }
            Err(_) => return check_failure("clang check exceeded 15 seconds", String::new()),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let diagnostics = parse_clang_diagnostics(&stderr);
        EbpfCheckResponse {
            ok: output.status.success(),
            message: if output.status.success() {
                "clang syntax check passed".to_string()
            } else {
                "clang reported compilation errors".to_string()
            },
            diagnostics,
            stdout,
            stderr,
        }
    }
}

fn selected_headers_cache_key(selected_headers: &[SelectedHeaderMetadata]) -> String {
    let mut signature = selected_headers
        .iter()
        .map(|header| format!("{}::{}::{}", header.id, header.include_hint, header.local_path))
        .collect::<Vec<_>>();
    signature.sort_unstable();
    source_cache_key(&signature.join("|"))
}

fn parse_clang_diagnostics(stderr: &str) -> Vec<EbpfCompilerDiagnostic> {
    stderr
        .lines()
        .filter_map(|line| {
            let marker = line.find("program.c:")? + "program.c:".len();
            let mut fields = line[marker..].splitn(4, ':');
            let line_number = fields.next()?.parse::<usize>().ok()?;
            let column = fields.next()?.parse::<usize>().ok()?;
            let severity = fields.next()?.trim();
            if !matches!(severity, "error" | "warning" | "note" | "fatal error") {
                return None;
            }
            let message = fields.next()?.trim().to_string();
            Some(EbpfCompilerDiagnostic {
                line: line_number.max(1),
                column: column.max(1),
                end_column: column.saturating_add(1).max(2),
                severity: if severity == "fatal error" { "error" } else { severity }.to_string(),
                message,
            })
        })
        .collect()
}

fn check_failure(message: &str, stderr: String) -> EbpfCheckResponse {
    EbpfCheckResponse {
        ok: false,
        message: message.to_string(),
        diagnostics: Vec::new(),
        stdout: String::new(),
        stderr,
    }
}

#[cfg(test)]
mod compiler_diagnostic_tests {
    use super::parse_clang_diagnostics;

    #[test]
    fn parses_clang_error_locations() {
        let parsed = parse_clang_diagnostics(
            "/tmp/cyanrex/program.c:7:12: error: use of undeclared identifier 'missing'\n",
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].line, 7);
        assert_eq!(parsed[0].column, 12);
        assert_eq!(parsed[0].severity, "error");
        assert!(parsed[0].message.contains("undeclared identifier"));
    }
}
