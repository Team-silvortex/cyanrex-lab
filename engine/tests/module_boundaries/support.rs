pub use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cyanrex_engine::{
    build_router, build_state,
    models::ebpf::{
        EbpfAttachmentDetail, EbpfCheckResponse, EbpfCompletionResponse, EbpfRunResponse,
    },
    services::{
        learning_store::LearningStore,
        runner_driver::*,
        runner_manager::{RunnerConfig, RunnerManager},
    },
    AppState,
};
use http_body_util::BodyExt;
pub use serde_json::{json, Value};
pub use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

pub const ORIGIN: &str = "http://localhost:3000";
pub const SOURCE: &str = "int synthetic_lab(void) { return 0; }";
pub static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub struct Fixture {
    pub root: PathBuf,
    pub state: Arc<AppState>,
    pub app: Router,
    pub calls: Arc<Mutex<Vec<(String, String, Vec<String>)>>>,
    env: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl Fixture {
    pub fn new() -> Self {
        assert!(
            std::env::var_os("DATABASE_URL").is_none(),
            "never use a deployment database"
        );
        let root =
            std::env::temp_dir().join(format!("cyanrex-module-boundary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let mut env = Vec::new();
        for (key, value) in [
            ("CYANREX_DATA_DIR", root.as_os_str().to_owned()),
            ("CYANREX_INSTANCE_ID", "default".into()),
            ("CYANREX_TEACHER_USERNAMES", "network-teacher".into()),
            ("CYANREX_CORS_ORIGINS", ORIGIN.into()),
            ("CYANREX_ALLOW_MISSING_ORIGIN", "false".into()),
            (
                "CYANREX_MODULES_DIR",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("modules")
                    .into_os_string(),
            ),
        ] {
            env.push((key, std::env::var_os(key)));
            std::env::set_var(key, value);
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut state = build_state();
        let inner = Arc::get_mut(&mut state).unwrap();
        inner.learning_store = LearningStore::with_local_data_path(root.join("learning.json"));
        inner.runner_manager = RunnerManager::new(
            RunnerConfig {
                max_concurrent: 2,
                max_per_user: 1,
                execution_timeout: std::time::Duration::from_secs(5),
                instance_id: "network-fixture".into(),
            },
            Arc::new(NonKernelDriver(calls.clone())),
        );
        let app = build_router(state.clone());
        Self {
            root,
            state,
            app,
            calls,
            env,
        }
    }

    pub async fn cookie(&self, username: &str) -> String {
        self.state
            .auth_service
            .register(username, "synthetic-network-password")
            .await
            .unwrap();
        let otp = self
            .state
            .auth_service
            .generate_current_totp_for_user(username)
            .unwrap();
        let login = self
            .state
            .auth_service
            .login(username, "synthetic-network-password", &otp)
            .await
            .unwrap();
        format!("cyanrex_session={}", login.token)
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        origin: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, origin);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    pub async fn get(&self, path: &str, cookie: &str) -> Value {
        let (status, body) = self
            .request("GET", path, Some(cookie), ORIGIN, Value::Null)
            .await;
        assert_eq!(status, StatusCode::OK);
        body
    }

    pub fn set_env(&mut self, key: &'static str, value: &str) {
        self.env.push((key, std::env::var_os(key)));
        std::env::set_var(key, value);
    }

    #[cfg(unix)]
    pub fn use_forged_downloader(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let bin = self.root.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let program = bin.join("wget");
        std::fs::write(
            &program,
            "#!/bin/sh\nprintf 'synthetic untrusted header\\n'\n",
        )
        .unwrap();
        std::fs::set_permissions(program, std::fs::Permissions::from_mode(0o700)).unwrap();
        self.env.push(("PATH", std::env::var_os("PATH")));
        std::env::set_var("PATH", bin);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for (key, original) in self.env.drain(..).rev() {
            match original {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        // Only this fixture's newly generated directory, never the caller's data directory.
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            eprintln!("fixture cleanup failed: {error}");
        }
    }
}

struct NonKernelDriver(Arc<Mutex<Vec<(String, String, Vec<String>)>>>);
impl RunnerDriver for NonKernelDriver {
    fn descriptor(&self) -> RunnerDriverDescriptor {
        RunnerDriverDescriptor {
            mode: "test-only",
            isolation: "synthetic-no-kernel",
        }
    }
    fn execute<'a>(&'a self, request: RunnerExecutionRequest<'a>) -> RunnerExecutionFuture<'a> {
        self.0.lock().unwrap().push((
            request.owner_username.into(),
            request.code.into(),
            request
                .selected_headers
                .iter()
                .map(|h| h.id.clone())
                .collect(),
        ));
        Box::pin(async {
            let mut result = EbpfRunResponse::validation_error(
                "synthetic compiler stopped before kernel loading",
            );
            result.stage = "compile".into();
            result
        })
    }
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
    fn list_attachments<'a>(
        &'a self,
        _: &'a str,
    ) -> RunnerOperationFuture<'a, Vec<EbpfAttachmentDetail>> {
        Box::pin(async { Err(RunnerOperationError::Unsupported) })
    }
    fn detach<'a>(
        &'a self,
        _: RunnerDetachRequest<'a>,
    ) -> RunnerOperationFuture<'a, RunnerDetachOutcome> {
        Box::pin(async { Err(RunnerOperationError::Unsupported) })
    }
}
