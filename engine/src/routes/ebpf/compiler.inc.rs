#[derive(Clone, Copy)]
enum CompilerOperation {
    Check,
    Completion,
}

struct CompilerMetricsGuard<'a> {
    state: &'a AppState,
    operation: CompilerOperation,
    started: Instant,
    finished: bool,
}

impl<'a> CompilerMetricsGuard<'a> {
    fn new(state: &'a AppState, operation: CompilerOperation) -> Self {
        match operation {
            CompilerOperation::Check => state.record_check_request(),
            CompilerOperation::Completion => state.record_completion_request(),
        }
        Self {
            state,
            operation,
            started: Instant::now(),
            finished: false,
        }
    }

    fn finish(mut self, cache_hit: Option<bool>, ok: bool, rejected: bool) {
        self.record(cache_hit, ok, rejected);
        self.finished = true;
    }

    fn record(&self, cache_hit: Option<bool>, ok: bool, rejected: bool) {
        let duration = self.started.elapsed().as_nanos() as u64;
        match self.operation {
            CompilerOperation::Check => self
                .state
                .finish_check_request(duration, cache_hit, ok, rejected),
            CompilerOperation::Completion => self
                .state
                .finish_completion_request(duration, cache_hit, ok, rejected),
        }
    }
}

impl Drop for CompilerMetricsGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.record(None, false, false);
        }
    }
}

fn check_failure_response(message: impl Into<String>) -> EbpfCheckResponse {
    EbpfCheckResponse {
        ok: false,
        message: message.into(),
        diagnostics: Vec::new(),
        stdout: String::new(),
        stderr: String::new(),
    }
}

fn completion_failure_response(message: impl Into<String>) -> EbpfCompletionResponse {
    EbpfCompletionResponse {
        ok: false,
        message: message.into(),
        items: Vec::new(),
    }
}
