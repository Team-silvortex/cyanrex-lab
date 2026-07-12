impl EbpfLoader {
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

        if !Self::supports_tracepoint_prog_attach().await {
            return Ok((
                false,
                "manual attach skipped: current bpftool does not support tracepoint attach via `bpftool prog attach`; upgrade bpftool or use host-side loader with libbpf".to_string(),
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

    async fn supports_tracepoint_prog_attach() -> bool {
        let output = match Command::new("bpftool")
            .arg("prog")
            .arg("help")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
        {
            Ok(output) => output,
            Err(_) => return false,
        };

        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_lowercase();

        let attach_type_block = match combined.find("attach_type := {") {
            Some(start) => {
                let tail = &combined[start..];
                let end = tail
                    .find('}')
                    .map(|idx| start + idx)
                    .unwrap_or(combined.len());
                &combined[start..end]
            }
            None => return false,
        };

        attach_type_block.contains("tracepoint")
            || attach_type_block.contains(" tp ")
            || attach_type_block.contains("| tp |")
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

    fn extract_function_names(code: &str) -> Vec<String> {
        let mut names = Vec::new();
        for line in code.lines() {
            let trimmed = line.trim();
            if !(trimmed.starts_with("int ") || trimmed.starts_with("static int ")) {
                continue;
            }
            let before_paren = match trimmed.split_once('(') {
                Some((left, _)) => left,
                None => continue,
            };
            let name = before_paren
                .split_whitespace()
                .last()
                .unwrap_or_default()
                .trim();
            if !name.is_empty() && name != "int" {
                names.push(name.to_string());
            }
        }
        names
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
