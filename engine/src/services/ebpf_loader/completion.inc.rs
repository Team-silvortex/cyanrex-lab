impl EbpfLoader {
    pub async fn complete(&self, code: &str, line: usize, column: usize) -> EbpfCompletionResponse {
        if code.trim().is_empty() || line == 0 || column == 0 {
            return completion_failure("source and one-based cursor position are required");
        }

        let temp_dir = std::env::temp_dir().join(format!("cyanrex-complete-{}", Uuid::new_v4()));
        if let Err(error) = fs::create_dir(&temp_dir).await {
            return completion_failure(&format!("failed to create completion directory: {error}"));
        }
        let response = self.complete_in_directory(code, line, column, &temp_dir).await;
        let _ = fs::remove_dir_all(&temp_dir).await;
        response
    }

    async fn complete_in_directory(
        &self,
        code: &str,
        line: usize,
        column: usize,
        temp_dir: &Path,
    ) -> EbpfCompletionResponse {
        let source_path = temp_dir.join("program.c");
        if let Err(error) = fs::write(&source_path, code).await {
            return completion_failure(&format!("failed to write source: {error}"));
        }
        if Self::requires_vmlinux_header(code) {
            if let Err(error) = Self::ensure_vmlinux_header(temp_dir).await {
                return completion_failure(&format!("failed to prepare vmlinux.h: {error}"));
            }
        }

        let completion_at = format!("{}:{line}:{column}", source_path.display());
        let mut command = Command::new(Self::resolve_clang_binary());
        command
            .kill_on_drop(true)
            .arg("-target")
            .arg("bpf")
            .arg("-fsyntax-only")
            .arg("-Xclang")
            .arg(format!("-code-completion-at={completion_at}"))
            .arg("-Xclang")
            .arg("-code-completion-macros")
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

        let output = match tokio::time::timeout(Duration::from_secs(8), command.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => return completion_failure(&format!("failed to run clang: {error}")),
            Err(_) => return completion_failure("clang completion exceeded 8 seconds"),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut items = parse_clang_completions(&stdout);
        items.extend(parse_clang_completions(&stderr));
        items.sort_by(|left, right| left.label.cmp(&right.label));
        items.dedup_by(|left, right| left.label == right.label && left.detail == right.detail);
        items.truncate(300);

        EbpfCompletionResponse {
            ok: !items.is_empty(),
            message: if items.is_empty() {
                "clang returned no semantic completions".to_string()
            } else {
                format!("{} semantic completions", items.len())
            },
            items,
        }
    }
}

fn parse_clang_completions(output: &str) -> Vec<EbpfCompletionItem> {
    output.lines().filter_map(parse_clang_completion_line).collect()
}

fn parse_clang_completion_line(line: &str) -> Option<EbpfCompletionItem> {
    let body = line.strip_prefix("COMPLETION: ")?;
    let (label, detail) = body.split_once(" : ").unwrap_or((body, body));
    let label = label.trim();
    if label.is_empty() || label.starts_with("Pattern") {
        return None;
    }
    let insert_text = detail
        .split("[#")
        .next()
        .unwrap_or(label)
        .trim()
        .to_string();
    let kind = if detail.contains("(") {
        "function"
    } else if detail.contains("struct ") || detail.contains("typedef") {
        "type"
    } else if label.chars().all(|character| !character.is_ascii_lowercase()) {
        "constant"
    } else {
        "field"
    };
    Some(EbpfCompletionItem {
        label: label.to_string(),
        insert_text: if insert_text.is_empty() { label.to_string() } else { insert_text },
        detail: detail.trim().to_string(),
        kind: kind.to_string(),
    })
}

fn completion_failure(message: &str) -> EbpfCompletionResponse {
    EbpfCompletionResponse { ok: false, items: Vec::new(), message: message.to_string() }
}

#[cfg(test)]
mod completion_parser_tests {
    use super::parse_clang_completion_line;

    #[test]
    fn parses_clang_completion_item() {
        let item = parse_clang_completion_line(
            "COMPLETION: bpf_ktime_get_ns : [#long long#]bpf_ktime_get_ns(<#void#>)",
        )
        .expect("completion should parse");
        assert_eq!(item.label, "bpf_ktime_get_ns");
        assert_eq!(item.kind, "function");
    }
}
