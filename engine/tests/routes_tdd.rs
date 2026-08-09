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
    std::env::set_var("CYANREX_ALLOW_REGISTRATION", "true");
    std::env::set_var("CYANREX_ALLOW_TOTP_BOOTSTRAP", "true");
    std::env::set_var("CYANREX_TEACHER_USERNAMES", "teacher");
    std::env::set_var(
        "CYANREX_RUNNER_AGENT_TOKEN",
        "test-runner-agent-token-32-bytes-minimum",
    );
    build_state()
}

include!("routes_tdd/basic.inc.rs");
include!("routes_tdd/modules.inc.rs");
include!("routes_tdd/auth.inc.rs");
include!("routes_tdd/auth_csrf.inc.rs");
include!("routes_tdd/learning.inc.rs");
include!("routes_tdd/runner.inc.rs");
include!("routes_tdd/runner_agent.inc.rs");
include!("routes_tdd/runner_job.inc.rs");
