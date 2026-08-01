use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cyanrex_engine::{build_router, build_state};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn test_state() -> std::sync::Arc<cyanrex_engine::AppState> {
    std::env::set_var("CYANREX_ALLOW_REGISTRATION", "true");
    std::env::set_var("CYANREX_ALLOW_TOTP_BOOTSTRAP", "true");
    std::env::set_var("CYANREX_TEACHER_USERNAMES", "teacher");
    build_state()
}

include!("routes_tdd/basic.inc.rs");
include!("routes_tdd/auth.inc.rs");
