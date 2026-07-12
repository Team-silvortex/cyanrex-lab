use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cyanrex_engine::{build_router, build_state};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

include!("routes_tdd/basic.inc.rs");
include!("routes_tdd/auth.inc.rs");
