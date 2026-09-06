mod runner_compiler_tests {
    use super::*;
    use cyanrex_engine::{
        models::ebpf::{
            EbpfAttachmentDetail, EbpfCheckResponse, EbpfCompilerDiagnostic, EbpfCompletionItem,
            EbpfCompletionResponse,
        },
        services::{
            runner_driver::{
                RunnerCheckRequest, RunnerCompilerOutcome, RunnerCompletionRequest,
                RunnerDetachOutcome, RunnerDetachRequest, RunnerDriver, RunnerDriverDescriptor,
                RunnerExecutionFuture, RunnerExecutionRequest, RunnerOperationError,
                RunnerOperationFuture,
            },
            runner_manager::{RunnerConfig, RunnerManager},
        },
    };
    use std::sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    };
    use std::time::Duration;

    const READY: u8 = 0;
    const UNAVAILABLE: u8 = 1;
    const UNSUPPORTED: u8 = 2;
    const PENDING: u8 = 3;

    struct CompilerDriver {
        behavior: AtomicU8,
        calls: Mutex<Vec<Value>>,
        entered: tokio::sync::Semaphore,
    }

    impl CompilerDriver {
        async fn readiness(&self) -> Result<(), RunnerOperationError> {
            match self.behavior.load(Ordering::Relaxed) {
                READY => Ok(()),
                UNAVAILABLE => Err(RunnerOperationError::Unavailable),
                UNSUPPORTED => Err(RunnerOperationError::Unsupported),
                _ => {
                    self.entered.add_permits(1);
                    std::future::pending().await
                }
            }
        }
    }

    impl RunnerDriver for CompilerDriver {
        fn descriptor(&self) -> RunnerDriverDescriptor {
            RunnerDriverDescriptor {
                mode: "compiler-test",
                isolation: "test_only",
            }
        }
        fn execute<'a>(&'a self, _: RunnerExecutionRequest<'a>) -> RunnerExecutionFuture<'a> {
            panic!("compiler requests must not load programs")
        }
        fn list_attachments<'a>(
            &'a self,
            _: &'a str,
        ) -> RunnerOperationFuture<'a, Vec<EbpfAttachmentDetail>> {
            panic!("compiler requests must not inspect attachments")
        }
        fn detach<'a>(
            &'a self,
            _: RunnerDetachRequest<'a>,
        ) -> RunnerOperationFuture<'a, RunnerDetachOutcome> {
            panic!("compiler requests must not detach programs")
        }
        fn check<'a>(
            &'a self,
            request: RunnerCheckRequest<'a>,
        ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCheckResponse>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(serde_json::json!({
                    "operation": "check", "owner": request.owner_username,
                    "code": request.code, "headers": request.selected_headers,
                }));
                self.readiness().await?;
                Ok(RunnerCompilerOutcome {
                    response: EbpfCheckResponse {
                        ok: false,
                        message: "driver diagnostic".into(),
                        diagnostics: vec![EbpfCompilerDiagnostic {
                            line: 2,
                            column: 3,
                            end_column: 4,
                            severity: "error".into(),
                            message: format!("diagnostic for {}", request.owner_username),
                        }],
                        stdout: "driver stdout".into(),
                        stderr: "driver stderr".into(),
                    },
                    cache_hit: (request.owner_username == "compiler-alice").then_some(true),
                })
            })
        }
        fn complete<'a>(
            &'a self,
            request: RunnerCompletionRequest<'a>,
        ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCompletionResponse>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(serde_json::json!({
                    "operation": "complete", "owner": request.owner_username,
                    "code": request.code, "headers": request.selected_headers,
                    "line": request.line, "column": request.column,
                }));
                self.readiness().await?;
                Ok(RunnerCompilerOutcome {
                    response: EbpfCompletionResponse {
                        ok: true,
                        message: "driver completions".into(),
                        items: vec![EbpfCompletionItem {
                            label: request.owner_username.into(),
                            insert_text: "item".into(),
                            detail: "driver detail".into(),
                            kind: "function".into(),
                        }],
                    },
                    cache_hit: (request.owner_username == "compiler-alice").then_some(false),
                })
            })
        }
    }

    struct Fixture {
        app: Router,
        state: Arc<cyanrex_engine::AppState>,
        driver: Arc<CompilerDriver>,
        cookies: Vec<String>,
    }
    impl Fixture {
        async fn new(behavior: u8, timeout: Duration) -> Self {
            let driver = Arc::new(CompilerDriver {
                behavior: AtomicU8::new(behavior),
                calls: Mutex::new(Vec::new()),
                entered: tokio::sync::Semaphore::new(0),
            });
            let mut state = test_state();
            Arc::get_mut(&mut state).unwrap().runner_manager = RunnerManager::new(
                RunnerConfig {
                    max_concurrent: 1,
                    max_per_user: 1,
                    execution_timeout: timeout,
                    instance_id: "compiler-test".into(),
                },
                driver.clone(),
            );
            let app = build_router(state.clone());
            let mut cookies = Vec::new();
            for owner in ["compiler-alice", "compiler-bob"] {
                register_user(&app, owner, "compiler-pass-123").await;
                let otp = state
                    .auth_service
                    .generate_current_totp_for_user(owner)
                    .unwrap();
                cookies.push(login_for_user(&app, owner, "compiler-pass-123", &otp).await);
            }
            Self {
                app,
                state,
                driver,
                cookies,
            }
        }
        async fn post(&self, path: &str, user: usize, body: Value) -> (StatusCode, Value) {
            post(
                self.app.clone(),
                path,
                Some(self.cookies[user].clone()),
                Some("http://localhost:3000"),
                body,
            )
            .await
        }
    }

    fn source() -> Value {
        serde_json::json!({"code": "int helper(void) { return 0; }", "line": 1, "column": 5,
            "owner_username": "forged-user", "selected_headers": [{"local_path": "/forged/header"}]})
    }
    async fn post(
        app: Router,
        path: &str,
        cookie: Option<String>,
        origin: Option<&str>,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        let response = app
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    #[tokio::test]
    async fn compiler_routes_delegate_source_owner_headers_cursor_and_cache_status() {
        let fixture = Fixture::new(READY, Duration::from_secs(45)).await;
        let selected = fixture
            .state
            .c_header_module
            .selected_metadata()
            .await
            .selected_headers;
        for (user, owner) in ["compiler-alice", "compiler-bob"].iter().enumerate() {
            let (status, body) = fixture.post("/ebpf/check", user, source()).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["ok"], false);
            assert_eq!(
                body["diagnostics"][0]["message"],
                format!("diagnostic for {owner}")
            );
            assert_eq!(body["stdout"], "driver stdout");
            let (status, body) = fixture.post("/ebpf/complete", user, source()).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["items"][0]["label"], *owner);
        }
        let calls = fixture.driver.calls.lock().unwrap();
        assert_eq!(calls.len(), 4);
        for (index, call) in calls.iter().enumerate() {
            assert_eq!(
                call["owner"],
                if index < 2 {
                    "compiler-alice"
                } else {
                    "compiler-bob"
                }
            );
            assert_eq!(call["code"], source()["code"]);
            assert_eq!(call["headers"], serde_json::to_value(&selected).unwrap());
            if index % 2 == 1 {
                assert_eq!(call["line"], 1);
                assert_eq!(call["column"], 5);
            }
        }
        let snapshot = fixture.state.performance_snapshot();
        assert_eq!(snapshot.check.total_requests, 2);
        assert_eq!(snapshot.check.cache_hits, 1);
        assert_eq!(snapshot.check.cache_misses, 0);
        assert_eq!(snapshot.check.errors, 2);
        assert_eq!(snapshot.check.in_flight, 0);
        assert_eq!(snapshot.completion.cache_hits, 0);
        assert_eq!(snapshot.completion.cache_misses, 1);
        assert_eq!(snapshot.completion.in_flight, 0);
    }

    #[tokio::test]
    async fn compiler_backend_failure_and_timeout_never_fall_back_to_local_compiler() {
        for (behavior, expected) in [
            (UNAVAILABLE, StatusCode::SERVICE_UNAVAILABLE),
            (UNSUPPORTED, StatusCode::SERVICE_UNAVAILABLE),
            (PENDING, StatusCode::REQUEST_TIMEOUT),
        ] {
            let fixture = Fixture::new(behavior, Duration::from_millis(25)).await;
            for path in ["/ebpf/check", "/ebpf/complete"] {
                let (status, body) = fixture.post(path, 0, source()).await;
                assert_eq!(status, expected);
                assert_eq!(body["ok"], false);
                assert!(body["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("runner")));
            }
            let snapshot = fixture.state.performance_snapshot();
            for metrics in [snapshot.check, snapshot.completion] {
                assert_eq!(metrics.errors, 1);
                assert_eq!(metrics.in_flight, 0);
                assert_eq!(metrics.cache_hits + metrics.cache_misses, 0);
            }
            assert_eq!(fixture.driver.calls.lock().unwrap().len(), 2);
        }
    }

    #[tokio::test]
    async fn compiler_guards_and_validation_run_before_driver_dispatch() {
        let fixture = Fixture::new(READY, Duration::from_secs(45)).await;
        for path in ["/ebpf/check", "/ebpf/complete"] {
            assert_eq!(
                post(
                    fixture.app.clone(),
                    path,
                    None,
                    Some("http://localhost:3000"),
                    source()
                )
                .await
                .0,
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                post(
                    fixture.app.clone(),
                    path,
                    Some(fixture.cookies[0].clone()),
                    Some("http://evil.example"),
                    source()
                )
                .await
                .0,
                StatusCode::FORBIDDEN
            );
            let mut oversized = source();
            oversized["code"] = Value::String("a".repeat(256 * 1024 + 1));
            assert_eq!(
                fixture.post(path, 0, oversized).await.0,
                StatusCode::PAYLOAD_TOO_LARGE
            );
        }
        for (line, column) in [(0, 1), (1, 0), (100_001, 1)] {
            let mut body = source();
            body["line"] = line.into();
            body["column"] = column.into();
            assert_eq!(
                fixture.post("/ebpf/complete", 0, body).await.0,
                StatusCode::BAD_REQUEST
            );
        }
        assert!(fixture.driver.calls.lock().unwrap().is_empty());
        assert_eq!(fixture.state.performance_snapshot().check.in_flight, 0);
        assert_eq!(fixture.state.performance_snapshot().completion.in_flight, 0);
    }

    #[tokio::test]
    async fn compiler_capacity_is_per_manager_and_cancel_releases_slots_and_metrics() {
        for (path, capacity) in [("/ebpf/check", 2), ("/ebpf/complete", 3)] {
            let fixture = Fixture::new(PENDING, Duration::from_secs(45)).await;
            let other = Fixture::new(READY, Duration::from_secs(45)).await;
            let mut tasks = Vec::new();
            for _ in 0..capacity {
                tasks.push(tokio::spawn(post(
                    fixture.app.clone(),
                    path,
                    Some(fixture.cookies[0].clone()),
                    Some("http://localhost:3000"),
                    source(),
                )));
            }
            tokio::time::timeout(
                Duration::from_secs(2),
                fixture.driver.entered.acquire_many(capacity),
            )
            .await
            .unwrap()
            .unwrap()
            .forget();
            let snapshot = fixture.state.performance_snapshot();
            let metrics = if path.ends_with("check") {
                snapshot.check
            } else {
                snapshot.completion
            };
            assert_eq!(metrics.in_flight, u64::from(capacity));
            let (status, body) = fixture.post(path, 1, source()).await;
            assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(body["ok"], false);
            assert_eq!(other.post(path, 0, source()).await.0, StatusCode::OK);
            for task in tasks {
                task.abort();
                assert!(task.await.unwrap_err().is_cancelled());
            }
            let snapshot = fixture.state.performance_snapshot();
            let metrics = if path.ends_with("check") {
                snapshot.check
            } else {
                snapshot.completion
            };
            assert_eq!(metrics.in_flight, 0);
            assert_eq!(metrics.rejected, 1);
            assert_eq!(
                fixture.driver.calls.lock().unwrap().len(),
                capacity as usize
            );
            fixture.driver.behavior.store(READY, Ordering::Relaxed);
            assert_eq!(fixture.post(path, 0, source()).await.0, StatusCode::OK);
        }
    }
}
