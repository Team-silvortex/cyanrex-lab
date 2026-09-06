mod runner_lifecycle_tests {
    use super::*;
    use cyanrex_engine::{
        models::ebpf::{
            EbpfAttachmentDetail, EbpfCheckResponse, EbpfCompletionResponse, EbpfRuntimeBackend,
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
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Copy)]
    enum Behavior {
        Ready,
        Unavailable,
        Pending,
    }

    struct TestDriver {
        calls: Mutex<Vec<(String, String)>>,
        behavior: Behavior,
    }

    impl TestDriver {
        async fn readiness(&self) -> Result<(), RunnerOperationError> {
            match self.behavior {
                Behavior::Ready => Ok(()),
                Behavior::Unavailable => Err(RunnerOperationError::Unavailable),
                Behavior::Pending => std::future::pending().await,
            }
        }
    }

    impl RunnerDriver for TestDriver {
        fn check<'a>(
            &'a self,
            _: RunnerCheckRequest<'a>,
        ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCheckResponse>> {
            Box::pin(async { Err(RunnerOperationError::Unsupported) })
        }
        fn complete<'a>(
            &'a self,
            _: RunnerCompletionRequest<'a>,
        ) -> RunnerOperationFuture<'a, RunnerCompilerOutcome<EbpfCompletionResponse>> {
            Box::pin(async { Err(RunnerOperationError::Unsupported) })
        }
        fn descriptor(&self) -> RunnerDriverDescriptor {
            RunnerDriverDescriptor {
                mode: "test_vm",
                isolation: "test_only",
            }
        }

        fn execute<'a>(&'a self, _: RunnerExecutionRequest<'a>) -> RunnerExecutionFuture<'a> {
            panic!("attachment requests must not execute a program")
        }

        fn list_attachments<'a>(
            &'a self,
            owner_username: &'a str,
        ) -> RunnerOperationFuture<'a, Vec<EbpfAttachmentDetail>> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .unwrap()
                    .push(("list".into(), owner_username.into()));
                self.readiness().await?;
                Ok(vec![EbpfAttachmentDetail {
                    pin_path: format!("vm://{owner_username}/experiment/attachment"),
                    source: format!("source for {owner_username}"),
                    program_name: "student-program".into(),
                }])
            })
        }

        fn detach<'a>(
            &'a self,
            request: RunnerDetachRequest<'a>,
        ) -> RunnerOperationFuture<'a, RunnerDetachOutcome> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .unwrap()
                    .push(("detach".into(), request.owner_username.into()));
                self.readiness().await?;
                let pin = format!("vm://{}/experiment/attachment", request.owner_username);
                if request.pin_path.is_some_and(|path| path != pin) {
                    return Err(RunnerOperationError::InvalidRequest(
                        "attachment is not owned by this user".into(),
                    ));
                }
                Ok(RunnerDetachOutcome {
                    detached: vec![pin],
                    clean: false,
                    safety_notes: vec!["execution environment cleanup not confirmed".into()],
                })
            })
        }
    }

    struct Fixture {
        app: Router,
        manager: RunnerManager,
        driver: Arc<TestDriver>,
        cookies: Vec<String>,
    }

    impl Fixture {
        async fn new(behavior: Behavior) -> Self {
            let driver = Arc::new(TestDriver {
                calls: Mutex::new(Vec::new()),
                behavior,
            });
            let manager = RunnerManager::new(
                RunnerConfig {
                    max_concurrent: 1,
                    max_per_user: 1,
                    execution_timeout: Duration::from_millis(30),
                    instance_id: "lifecycle-test".into(),
                },
                driver.clone(),
            );
            let mut state = test_state();
            Arc::get_mut(&mut state).unwrap().runner_manager = manager.clone();
            let app = build_router(state.clone());
            let mut cookies = Vec::new();
            for username in ["lifecycle-alice", "lifecycle-bob"] {
                register_user(&app, username, "lifecycle-pass-123").await;
                let otp = state
                    .auth_service
                    .generate_current_totp_for_user(username)
                    .unwrap();
                cookies.push(login_for_user(&app, username, "lifecycle-pass-123", &otp).await);
            }
            Self {
                app,
                manager,
                driver,
                cookies,
            }
        }

        async fn request(
            &self,
            path: &str,
            cookie: Option<&str>,
            body: Option<Value>,
            origin: Option<&str>,
        ) -> (StatusCode, Value) {
            let mut request = Request::builder().uri(path);
            if let Some(cookie) = cookie {
                request = request.header(header::COOKIE, cookie);
            }
            if let Some(origin) = origin {
                request = request.header(header::ORIGIN, origin);
            }
            let body = if let Some(body) = body {
                request = request
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json");
                Body::from(body.to_string())
            } else {
                Body::empty()
            };
            let response = self
                .app
                .clone()
                .oneshot(request.body(body).unwrap())
                .await
                .unwrap();
            let status = response.status();
            let body = response.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
        }
    }

    #[tokio::test]
    async fn attachment_inventory_uses_session_owner_and_outlives_execution_lease() {
        let fixture = Fixture::new(Behavior::Ready).await;
        let lease = fixture
            .manager
            .try_acquire("lifecycle-alice", EbpfRuntimeBackend::Aya)
            .unwrap();
        drop(lease);
        assert_eq!(
            fixture.manager.status_for("lifecycle-alice").active_total,
            0
        );
        for (index, owner) in ["lifecycle-alice", "lifecycle-bob"].iter().enumerate() {
            let cookie = Some(fixture.cookies[index].as_str());
            let (status, body) = fixture
                .request("/ebpf/attachments", cookie, None, None)
                .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                body["pin_paths"],
                serde_json::json!([format!("vm://{owner}/experiment/attachment")])
            );
            let (status, body) = fixture
                .request("/ebpf/attachments/details", cookie, None, None)
                .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                body["attachments"][0]["source"],
                format!("source for {owner}")
            );
            assert_eq!(body["attachments"].as_array().unwrap().len(), 1);
        }
        assert_eq!(
            *fixture.driver.calls.lock().unwrap(),
            vec![
                ("list".into(), "lifecycle-alice".into()),
                ("list".into(), "lifecycle-alice".into()),
                ("list".into(), "lifecycle-bob".into()),
                ("list".into(), "lifecycle-bob".into()),
            ]
        );
    }

    #[tokio::test]
    async fn detach_uses_driver_ownership_and_cleanup_report_without_host_inspection() {
        let fixture = Fixture::new(Behavior::Ready).await;
        let cookie = Some(fixture.cookies[0].as_str());
        let origin = Some("http://localhost:3000");
        let (status, body) = fixture.request("/ebpf/detach", cookie, Some(serde_json::json!({
            "pin_path": "vm://lifecycle-bob/experiment/attachment", "owner_username": "lifecycle-bob"
        })), origin).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["ok"], false);
        for pin_path in [Some("vm://lifecycle-alice/experiment/attachment"), None] {
            let (status, body) = fixture
                .request(
                    "/ebpf/detach",
                    cookie,
                    Some(serde_json::json!({"pin_path": pin_path})),
                    origin,
                )
                .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["ok"], true);
            assert_eq!(body["clean"], false);
            assert_eq!(
                body["safety_notes"],
                serde_json::json!(["execution environment cleanup not confirmed"])
            );
            assert_eq!(
                body["detached"],
                serde_json::json!(["vm://lifecycle-alice/experiment/attachment"])
            );
        }
        assert_eq!(
            *fixture.driver.calls.lock().unwrap(),
            vec![("detach".into(), "lifecycle-alice".into()); 3]
        );
    }

    async fn assert_backend_failure(behavior: Behavior, expected: StatusCode) {
        let fixture = Fixture::new(behavior).await;
        let cookie = Some(fixture.cookies[0].as_str());
        for path in ["/ebpf/attachments", "/ebpf/attachments/details"] {
            let (status, body) = fixture.request(path, cookie, None, None).await;
            assert_eq!(status, expected);
            assert!(body.get("pin_paths").is_none());
            assert!(body.get("attachments").is_none());
        }
        let (status, body) = fixture
            .request(
                "/ebpf/detach",
                cookie,
                Some(serde_json::json!({})),
                Some("http://localhost:3000"),
            )
            .await;
        assert_eq!(status, expected);
        assert_eq!(body["ok"], false);
        assert_eq!(body["clean"], false);
        assert_eq!(fixture.driver.calls.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn attachment_backend_unavailability_never_falls_back_to_local_loader() {
        assert_backend_failure(Behavior::Unavailable, StatusCode::SERVICE_UNAVAILABLE).await;
    }

    #[tokio::test]
    async fn attachment_operations_have_bounded_deadlines() {
        assert_backend_failure(Behavior::Pending, StatusCode::REQUEST_TIMEOUT).await;
    }

    #[tokio::test]
    async fn attachment_routes_keep_auth_and_csrf_guards_before_driver() {
        let fixture = Fixture::new(Behavior::Ready).await;
        for path in ["/ebpf/attachments", "/ebpf/attachments/details"] {
            assert_eq!(
                fixture.request(path, None, None, None).await.0,
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            fixture
                .request(
                    "/ebpf/detach",
                    None,
                    Some(serde_json::json!({})),
                    Some("http://localhost:3000")
                )
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            fixture
                .request(
                    "/ebpf/detach",
                    Some(&fixture.cookies[0]),
                    Some(serde_json::json!({})),
                    Some("http://evil.example")
                )
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert!(fixture.driver.calls.lock().unwrap().is_empty());
    }
}
