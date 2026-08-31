const MAX_DEBUG_BREAKPOINTS: usize = 16;
const DEBUG_TRACE_PREFIX: &str = "cyanrex_bp:";

struct DebugInstrumentation {
    source: String,
    info: EbpfDebugInfo,
}

#[derive(Clone, Copy, Default)]
struct CScanState {
    block_comment: bool,
    quote: Option<u8>,
    escaped: bool,
}

struct DebugLineContext {
    sanitized: Vec<u8>,
    in_function: bool,
    function_entry_brace: Option<usize>,
    paren_depth_at_start: usize,
}

impl DebugInstrumentation {
    fn disable_instrumentation(&mut self, reason: &str, original_source: &str) {
        for line in self.info.instrumented_lines.drain(..) {
            self.info.rejected.push(EbpfDebugRejectedBreakpoint {
                line,
                reason: reason.to_string(),
            });
        }
        self.info.rejected.sort_by_key(|item| item.line);
        self.info.session_id = None;
        self.source = original_source.to_string();
    }
}

impl EbpfLoader {
    pub async fn run(
        &self,
        owner_username: &str,
        code: &str,
        program_name: Option<&str>,
        runtime_backend: EbpfRuntimeBackend,
        selected_headers: &[SelectedHeaderMetadata],
        debug_breakpoints: Option<&[u32]>,
    ) -> EbpfRunResponse {
        let mut debug = Self::instrument_debug_breakpoints(code, debug_breakpoints.unwrap_or(&[]));
        let mut response = self
            .run_once(
                owner_username,
                &debug.source,
                program_name,
                runtime_backend,
                selected_headers,
            )
            .await;

        if !response.success
            && response.stage == "compile"
            && response.message == "clang failed to compile eBPF source"
            && !debug.info.instrumented_lines.is_empty()
        {
            debug.disable_instrumentation(
                "debug instrumentation was disabled because the instrumented source did not compile",
                code,
            );
            response = self
                .run_once(
                    owner_username,
                    code,
                    program_name,
                    runtime_backend,
                    selected_headers,
                )
                .await;
        }

        if response.success {
            if let Some(pin_path) = response.pin_path.as_deref() {
                if let Some(record) = self.attachments.write().await.get_mut(pin_path) {
                    record.source = code.to_string();
                }
            }
        }

        if !debug.info.requested_lines.is_empty() {
            response.debug = Some(debug.info);
        }
        response
    }

    fn instrument_debug_breakpoints(code: &str, requested: &[u32]) -> DebugInstrumentation {
        let mut requested_lines = requested.to_vec();
        requested_lines.sort_unstable();
        requested_lines.dedup();

        let session_id = Uuid::new_v4().simple().to_string();
        let session_id = session_id[..12].to_string();
        let lines = source_lines(code);
        let contexts = analyze_debug_line_contexts(&lines);
        let helper_available = code.contains("bpf_helpers.h") || code.contains("bpf_printk");
        let mut rejected = Vec::new();
        let mut insertions = HashMap::<usize, (usize, String)>::new();

        for (position, line) in requested_lines.iter().copied().enumerate() {
            let reject = |reason: &str| EbpfDebugRejectedBreakpoint {
                line,
                reason: reason.to_string(),
            };

            if position >= MAX_DEBUG_BREAKPOINTS {
                rejected.push(reject("the per-run breakpoint limit is 16"));
                continue;
            }
            if line == 0 || line as usize > lines.len() {
                rejected.push(reject("line is outside the source file"));
                continue;
            }
            if !helper_available {
                rejected.push(reject(
                    "bpf_printk is unavailable; include <bpf/bpf_helpers.h>",
                ));
                continue;
            }

            let index = line as usize - 1;
            match debug_insertion_offset(&lines[index], &contexts[index]) {
                Ok(offset) => {
                    let marker = format!(
                        "bpf_printk(\"{DEBUG_TRACE_PREFIX}{session_id}:{line}\"); "
                    );
                    insertions.insert(index, (offset, marker));
                }
                Err(reason) => rejected.push(reject(reason)),
            }
        }

        let mut source = String::with_capacity(code.len() + insertions.len() * 64);
        let mut instrumented_lines = Vec::new();
        for (index, original) in lines.iter().enumerate() {
            if let Some((offset, marker)) = insertions.get(&index) {
                source.push_str(&original[..*offset]);
                source.push_str(marker);
                source.push_str(&original[*offset..]);
                instrumented_lines.push(index as u32 + 1);
            } else {
                source.push_str(original);
            }
        }

        DebugInstrumentation {
            source,
            info: EbpfDebugInfo {
                mode: "kernel-trace".to_string(),
                session_id: (!instrumented_lines.is_empty()).then_some(session_id),
                requested_lines,
                instrumented_lines,
                rejected,
            },
        }
    }
}

fn source_lines(code: &str) -> Vec<&str> {
    let mut lines = code.split_inclusive('\n').collect::<Vec<_>>();
    if lines.is_empty() || code.ends_with('\n') {
        lines.push("");
    }
    lines
}

fn analyze_debug_line_contexts(lines: &[&str]) -> Vec<DebugLineContext> {
    let mut scan_state = CScanState::default();
    let mut brace_depth = 0_usize;
    let mut function_depth = None;
    let mut function_paren_depth = 0_usize;
    let mut top_level_fragment = Vec::new();
    let mut preprocessor_continuation = false;
    let mut contexts = Vec::with_capacity(lines.len());

    for line in lines {
        let sanitized = sanitize_c_line(line, &mut scan_state);
        let in_function = function_depth.is_some();
        let paren_depth_at_start = function_paren_depth;
        let mut function_entry_brace = None;
        let is_preprocessor = preprocessor_continuation
            || sanitized
                .iter()
                .find(|byte| !byte.is_ascii_whitespace())
                == Some(&b'#');
        preprocessor_continuation = is_preprocessor
            && line
                .trim_end_matches(['\r', '\n'])
                .trim_end()
                .ends_with('\\');

        if is_preprocessor {
            if function_depth.is_none() && brace_depth == 0 {
                top_level_fragment.clear();
            }
            contexts.push(DebugLineContext {
                sanitized,
                in_function,
                function_entry_brace,
                paren_depth_at_start,
            });
            continue;
        }

        for (index, byte) in sanitized.iter().copied().enumerate() {
            if function_depth.is_none() && brace_depth == 0 {
                match byte {
                    b';' => top_level_fragment.clear(),
                    b'{' => {
                        if is_function_header(&top_level_fragment) {
                            function_depth = Some(1);
                            function_paren_depth = 0;
                            function_entry_brace = Some(index);
                        }
                        top_level_fragment.clear();
                    }
                    _ => top_level_fragment.push(byte),
                }
            }

            if function_depth.is_some() {
                match byte {
                    b'(' => function_paren_depth += 1,
                    b')' => function_paren_depth = function_paren_depth.saturating_sub(1),
                    _ => {}
                }
            }

            match byte {
                b'{' => brace_depth += 1,
                b'}' => {
                    let closes_function = function_depth == Some(brace_depth);
                    brace_depth = brace_depth.saturating_sub(1);
                    if closes_function {
                        function_depth = None;
                        function_paren_depth = 0;
                        top_level_fragment.clear();
                    }
                }
                _ => {}
            }
        }

        contexts.push(DebugLineContext {
            sanitized,
            in_function,
            function_entry_brace,
            paren_depth_at_start,
        });
    }

    contexts
}

fn sanitize_c_line(line: &str, state: &mut CScanState) -> Vec<u8> {
    let input = line.as_bytes();
    let mut output = input.to_vec();
    let mut index = 0_usize;

    while index < input.len() {
        if state.block_comment {
            output[index] = b' ';
            if input[index] == b'*' && input.get(index + 1) == Some(&b'/') {
                output[index + 1] = b' ';
                state.block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if let Some(quote) = state.quote {
            output[index] = b' ';
            if state.escaped {
                state.escaped = false;
            } else if input[index] == b'\\' {
                state.escaped = true;
            } else if input[index] == quote {
                state.quote = None;
            }
            index += 1;
            continue;
        }

        if input[index] == b'/' && input.get(index + 1) == Some(&b'/') {
            output[index..].fill(b' ');
            break;
        }
        if input[index] == b'/' && input.get(index + 1) == Some(&b'*') {
            output[index] = b' ';
            output[index + 1] = b' ';
            state.block_comment = true;
            index += 2;
            continue;
        }
        if input[index] == b'\'' || input[index] == b'"' {
            state.quote = Some(input[index]);
            state.escaped = false;
            output[index] = b' ';
        } else if !input[index].is_ascii() {
            output[index] = b' ';
        }
        index += 1;
    }

    output
}

fn is_function_header(fragment: &[u8]) -> bool {
    let text = String::from_utf8_lossy(fragment);
    let trimmed = text.trim();
    if !trimmed.contains(')') || trimmed.contains('=') {
        return false;
    }
    !["struct", "union", "enum", "typedef"]
        .iter()
        .any(|keyword| trimmed.starts_with(keyword))
}

fn debug_insertion_offset(line: &str, context: &DebugLineContext) -> Result<usize, &'static str> {
    if let Some(brace) = context.function_entry_brace {
        return Ok(brace + 1);
    }
    if !context.in_function {
        return Err("line is outside an executable function");
    }
    if context.paren_depth_at_start > 0 {
        return Err("line continues a multi-line expression");
    }

    let first = context
        .sanitized
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .ok_or("line has no executable code")?;
    let sanitized = String::from_utf8_lossy(&context.sanitized[first..]);
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        return Err("line has no executable code");
    }
    if line[first..].starts_with('#') {
        return Err("preprocessor directives cannot host breakpoints");
    }
    if trimmed == ";" {
        return Err("empty statements cannot host breakpoints");
    }

    if trimmed.starts_with('}') || trimmed.starts_with("else") {
        return context.sanitized[first..]
            .iter()
            .position(|byte| *byte == b'{')
            .map(|offset| first + offset + 1)
            .ok_or("control-flow boundary has no safe probe position");
    }
    if trimmed.starts_with('{') {
        return Ok(first + 1);
    }
    if trimmed.starts_with("case ") || trimmed.starts_with("default:") || is_c_label(trimmed) {
        return context.sanitized[first..]
            .iter()
            .position(|byte| *byte == b':')
            .map(|offset| first + offset + 1)
            .ok_or("label has no safe probe position");
    }
    if matches!(
        context.sanitized[first],
        b')' | b']' | b'.' | b',' | b':' | b'?' | b'+' | b'-' | b'/' | b'%' | b'&' | b'|' | b'^'
    ) {
        return Err("line appears to continue the previous statement");
    }

    Ok(first)
}

fn is_c_label(line: &str) -> bool {
    let Some(colon) = line.find(':') else {
        return false;
    };
    let candidate = line[..colon].trim();
    !candidate.is_empty()
        && candidate
            .bytes()
            .enumerate()
            .all(|(index, byte)| byte == b'_' || byte.is_ascii_alphanumeric() && (index > 0 || !byte.is_ascii_digit()))
}

#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn instruments_function_entry_and_statement_without_changing_line_count() {
        let source = "#include <bpf/bpf_helpers.h>\nSEC(\"xdp\")\nint demo(void *ctx) {\n  return 2;\n}\n";
        let debug = EbpfLoader::instrument_debug_breakpoints(source, &[3, 4]);

        assert_eq!(debug.info.instrumented_lines, vec![3, 4]);
        assert_eq!(source_lines(source).len(), source_lines(&debug.source).len());
        assert!(debug.source.contains("{bpf_printk(\"cyanrex_bp:"));
        assert!(debug.source.contains("bpf_printk(\"cyanrex_bp:"));
    }

    #[test]
    fn rejects_global_comment_and_out_of_range_lines() {
        let source = "#include <bpf/bpf_helpers.h>\n// comment\nint demo(void *ctx) {\n  return 0;\n}\n";
        let debug = EbpfLoader::instrument_debug_breakpoints(source, &[1, 2, 9]);

        assert!(debug.info.instrumented_lines.is_empty());
        assert_eq!(debug.info.rejected.len(), 3);
        assert!(debug.info.session_id.is_none());
    }

    #[test]
    fn does_not_treat_map_initializer_as_a_function() {
        let source = "#include <bpf/bpf_helpers.h>\nstruct map_def SEC(\".maps\") values = {\n  int type;\n};\n";
        let debug = EbpfLoader::instrument_debug_breakpoints(source, &[2, 3]);

        assert!(debug.info.instrumented_lines.is_empty());
        assert_eq!(debug.info.rejected.len(), 2);
    }

    #[test]
    fn ignores_braces_and_assignments_in_preprocessor_macros() {
        let source = "#include <bpf/bpf_helpers.h>\n#define WRAP(value) ({ int x = value; x; })\nint demo(void *ctx) {\n  return 0;\n}\n";
        let debug = EbpfLoader::instrument_debug_breakpoints(source, &[4]);

        assert_eq!(debug.info.instrumented_lines, vec![4]);
        assert!(debug.info.rejected.is_empty());
    }

    #[test]
    fn enforces_breakpoint_limit() {
        let mut source = "#include <bpf/bpf_helpers.h>\nint demo(void *ctx) {\n".to_string();
        for _ in 0..20 {
            source.push_str("  ctx = ctx;\n");
        }
        source.push_str("  return 0;\n}\n");
        let requested = (3..=20).collect::<Vec<_>>();
        let debug = EbpfLoader::instrument_debug_breakpoints(&source, &requested);

        assert_eq!(debug.info.instrumented_lines.len(), MAX_DEBUG_BREAKPOINTS);
        assert_eq!(debug.info.rejected.len(), 2);
    }
}
