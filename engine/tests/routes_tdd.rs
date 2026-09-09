use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cyanrex_engine::{build_router, build_state};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

static CSRF_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn test_state() -> std::sync::Arc<cyanrex_engine::AppState> {
    let module_catalog = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Engine should have a repository parent")
        .join("modules");
    std::env::set_var("CYANREX_MODULES_DIR", module_catalog);
    std::env::set_var("CYANREX_ALLOW_REGISTRATION", "true");
    std::env::set_var("CYANREX_ALLOW_TOTP_BOOTSTRAP", "true");
    std::env::set_var("CYANREX_TEACHER_USERNAMES", "teacher");
    std::env::set_var("CYANREX_ADMIN_USERNAMES", "legacy-admin");
    std::env::set_var(
        "CYANREX_RUNNER_AGENT_TOKEN",
        "test-runner-agent-token-32-bytes-minimum",
    );
    build_state()
}

fn module_manifest_version(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Engine should have a repository parent")
        .join("modules")
        .join(name)
        .join("module.json");
    let source = std::fs::read(path).expect("module manifest should be readable");
    let manifest: Value = serde_json::from_slice(&source).expect("module manifest should be JSON");
    manifest["version"]
        .as_str()
        .expect("module manifest version should be a string")
        .to_owned()
}

include!("routes_tdd/basic.inc.rs");
include!("routes_tdd/events_ws.inc.rs");
include!("routes_tdd/modules.inc.rs");
include!("routes_tdd/command.inc.rs");
include!("routes_tdd/auth.inc.rs");
include!("routes_tdd/auth_csrf.inc.rs");
include!("routes_tdd/teacher_authority.inc.rs");
include!("routes_tdd/learning.inc.rs");
include!("routes_tdd/learning_feedback.inc.rs");
include!("routes_tdd/learning_resume.inc.rs");
include!("routes_tdd/runner.inc.rs");
include!("routes_tdd/runner_lifecycle.inc.rs");
include!("routes_tdd/runner_compiler.inc.rs");
include!("routes_tdd/runner_agent.inc.rs");
include!("routes_tdd/runner_job.inc.rs");
